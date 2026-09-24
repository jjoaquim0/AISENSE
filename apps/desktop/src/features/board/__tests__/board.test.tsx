import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BoardEvent } from '@/types/generated/BoardEvent';
import type { BoardView } from '@/types/generated/BoardView';
import type { CardView } from '@/types/generated/CardView';
import type { Column } from '@/types/generated/Column';

const api = {
  get: vi.fn(),
  changes: vi.fn(),
  move: vi.fn(),
  show: vi.fn(),
  comment: vi.fn(),
  saveColumns: vi.fn(),
  automationsToml: vi.fn(),
};
let emit: (event: BoardEvent) => void = () => {};
vi.mock('@/features/board/api', () => ({
  boardApi: new Proxy(
    {},
    {
      get:
        (_, key: string) =>
        (...a: unknown[]) =>
          api[key as keyof typeof api]?.(...a),
    },
  ),
  onBoardChanged: (handler: (event: BoardEvent) => void) => {
    emit = handler;
    return Promise.resolve(() => {});
  },
}));
vi.mock('@/features/skills/MarkdownPreview', () => ({
  MarkdownPreview: ({ markdown }: { markdown: string }) => <p>{markdown}</p>,
}));

const { BoardScreen } = await import('../BoardScreen');
const { CardDetailPanel } = await import('../CardDetailPanel');
const { ColumnsEditor } = await import('../ColumnsEditor');
const model = await import('../boardModel');
const automations = await import('../automationModel');
const columnsModel = await import('../columnsModel');

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const col = (slug: string, kind: Column['kind'], over: Partial<Column> = {}): Column => ({
  id: `col_${slug}`,
  boardId: 'brd_1',
  slug,
  name: slug.toUpperCase(),
  kind,
  wipLimit: null,
  wipPerAgent: null,
  position: 0,
  requiresApproval: false,
  approverMustDiffer: true,
  requiresCommands: [],
  ...over,
});

const card = (id: string, column: Column, over: Partial<CardView> = {}): CardView => ({
  id,
  teamId: 't1',
  columnId: column.id,
  columnSlug: column.slug,
  title: `Cartão ${id}`,
  body: '',
  assignee: null,
  assigneeHandle: null,
  createdBy: null,
  parentId: null,
  position: 0,
  priority: 'normal',
  labels: [],
  checklist: [],
  links: [],
  blockReason: null,
  version: 1,
  archivedAt: null,
  approvedBy: null,
  approvedAt: null,
  columnSince: 0,
  createdAt: 0,
  updatedAt: 0,
  blockedBy: [],
  comments: 0,
  ...over,
});

const todo = col('todo', 'ready');
const doing = col('doing', 'active', { wipLimit: 1 });
const done = col('done', 'terminal');
const board = (cards: CardView[]): BoardView => ({
  teamId: 't1',
  teamName: 'Squad',
  board: { id: 'brd_1', teamId: 't1', automations: [], createdAt: 0 },
  columns: [todo, doing, done],
  cards,
  agents: [{ id: 'a1', handle: 'backend', color: 'indigo' }],
  now: 0,
});

describe('regras da tela do quadro', () => {
  it('agrupa, filtra, conta e move otimista', () => {
    const view = board([
      card('tsk_a', todo, { labels: ['api'] }),
      card('tsk_b', doing, { assigneeHandle: 'backend' }),
    ]);
    expect(
      model
        .byColumn(view)
        .get(doing.id)
        ?.map((c) => c.id),
    ).toEqual(['tsk_b']);
    expect(model.countLabel(doing, 1)).toBe('1/1');
    expect(model.countLabel(todo, 3)).toBe('3');
    expect(model.applyFilter(view.cards, { assignee: 'none', label: null })).toHaveLength(1);
    expect(model.applyFilter(view.cards, { assignee: null, label: 'api' })[0]?.id).toBe('tsk_a');
    const moved = model.moveLocally(view, 'tsk_a', done);
    expect(moved.cards[0]?.columnSlug).toBe('done');
    expect(view.cards[0]?.columnSlug).toBe('todo');
    expect(model.changedCards([{ cardId: 'x' }, { cardId: 'x' }, { cardId: 'y' }] as never)).toBe(
      2,
    );
    expect(model.shortId('tsk_01J8XABCDEF123456')).toBe('tsk_123456');
    expect(model.ago(120_000, 0)).toBe('há 2min');
  });

  it('automações: a variante é a chave; colunas: slug e destino obrigatório', () => {
    expect(automations.actionKind({ move: 'done' })).toBe('move');
    expect(automations.actionKind({ unblock_dependents: true })).toBe('unblock_dependents');
    expect(automations.blankAction('notify')).toEqual({ notify: 'assignee', message: '' });
    expect(columnsModel.slugify('Em Revisão!')).toBe('em-revisao');
    const view = board([card('tsk_a', todo)]);
    const drafts = columnsModel.draftsOf(view.columns).filter((d) => d.slug !== 'todo');
    expect(columnsModel.pendingMoves(view, drafts)).toEqual([
      { slug: 'todo', name: 'TODO', count: 1 },
    ]);
  });
});

describe('<BoardScreen />', () => {
  let container: HTMLDivElement;
  let root: Root;
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    api.changes.mockResolvedValue([]);
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  const drop = async (cardId: string, slug: string) => {
    const data = new Map<string, string>([['application/x-aisense-card', cardId]]);
    const event = new Event('drop', { bubbles: true, cancelable: true });
    Object.assign(event, {
      dataTransfer: { getData: (t: string) => data.get(t) ?? '', types: [...data.keys()] },
    });
    await act(async () => {
      container.querySelector(`[data-column="${slug}"]`)?.dispatchEvent(event);
    });
  };
  const columnOf = (cardId: string) =>
    container
      .querySelector(`[data-card="${cardId}"]`)
      ?.closest('[data-column]')
      ?.getAttribute('data-column');

  it('arrastar para coluna cheia mostra o erro da CLI e desfaz o movimento', async () => {
    api.get.mockResolvedValue(
      board([card('tsk_a', todo), card('tsk_b', doing, { assigneeHandle: 'backend' })]),
    );
    let reject: (e: unknown) => void = () => {};
    api.move.mockReturnValue(new Promise((_, r) => (reject = r)));
    await act(async () => root.render(<BoardScreen teamId="t1" />));
    expect(container.textContent).toContain('(1/1)');
    expect(container.querySelector('[data-card="tsk_b"]')?.getAttribute('style')).toContain(
      '--agent-indigo',
    );

    await drop('tsk_a', 'doing');
    // Otimista: já está em Fazendo enquanto o core decide.
    expect(columnOf('tsk_a')).toBe('doing');
    expect(api.move).toHaveBeenCalledWith('t1', 'tsk_a', 'doing', undefined);
    await act(async () =>
      reject({
        code: 'wip_exceeded',
        message: 'Fazendo está no limite (1/1). Conclua ou devolva um cartão antes de pegar outro.',
        hint: null,
      }),
    );
    expect(columnOf('tsk_a')).toBe('todo');
    expect(container.querySelector('[role="alert"]')?.textContent).toBe(
      'Fazendo está no limite (1/1). Conclua ou devolva um cartão antes de pegar outro.',
    );
  });

  it('mudança vinda da CLI relê e realça o cartão por 400 ms', async () => {
    vi.useFakeTimers();
    api.get.mockResolvedValue(board([card('tsk_a', todo)]));
    await act(async () => root.render(<BoardScreen teamId="t1" />));
    await act(async () =>
      emit({
        teamId: 't1',
        cardId: 'tsk_a',
        action: 'moved',
        actor: { kind: 'system' },
        involved: [],
        at: 0,
      }),
    );
    expect(api.get).toHaveBeenCalledTimes(2);
    expect(container.querySelector('[data-card="tsk_a"]')?.className).toContain('bg-active');
    await act(async () => vi.advanceTimersByTime(model.HIGHLIGHT_MS));
    expect(container.querySelector('[data-card="tsk_a"]')?.className).not.toContain('bg-active');
    vi.useRealTimers();
  });
});

describe('<CardDetailPanel /> e <ColumnsEditor />', () => {
  let root: Root;
  let container: HTMLDivElement;
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
    vi.clearAllMocks();
  });

  it('comentar pela UI manda para o core (que avisa o responsável)', async () => {
    const view = card('tsk_a', doing, { assigneeHandle: 'backend', body: 'corpo **md**' });
    api.show.mockResolvedValue({
      card: view,
      column: doing,
      comments: [
        {
          id: 'cmt_1',
          cardId: 'tsk_a',
          author: { kind: 'agent', agentId: 'a1' },
          body: 'subi o commit',
          createdAt: 0,
        },
      ],
      activity: [],
      dependsOn: [{ id: 'tsk_z', title: 'Antes', columnSlug: 'todo', open: true }],
      dependents: [],
      children: [],
      agents: [{ id: 'a1', handle: 'backend', color: 'indigo' }],
    });
    api.comment.mockResolvedValue({});
    const open = vi.fn();
    await act(async () =>
      root.render(
        <CardDetailPanel
          teamId="t1"
          cardId="tsk_a"
          columns={[todo, doing, done]}
          agents={[{ id: 'a1', handle: 'backend', color: 'indigo' }]}
          version={1}
          onOpenCard={open}
          onClose={() => {}}
        />,
      ),
    );
    expect(document.body.textContent).toContain('subi o commit');
    expect(document.body.textContent).toContain('@backend');
    const textarea = document.querySelector(
      'textarea[aria-label="Comentário"]',
    ) as HTMLTextAreaElement;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(
        textarea,
        'o contrato mudou',
      );
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await act(async () => {
      textarea
        .closest('form')
        ?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    });
    expect(api.comment).toHaveBeenCalledWith('t1', 'tsk_a', 'o contrato mudou');
    // Dependência navegável.
    const dep = [...document.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Antes'),
    );
    await act(async () => dep?.click());
    expect(open).toHaveBeenCalledWith('tsk_z');
  });

  it('remover coluna com cartões exige escolher o destino', async () => {
    api.saveColumns.mockResolvedValue([]);
    const view = board([card('tsk_a', todo)]);
    await act(async () =>
      root.render(<ColumnsEditor teamId="t1" view={view} onClose={() => {}} onSaved={() => {}} />),
    );
    const remove = document.querySelector('button[aria-label="Remover TODO"]') as HTMLButtonElement;
    await act(async () => remove.click());
    const save = [...document.querySelectorAll('button')].find(
      (b) => b.textContent === 'Salvar colunas',
    ) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    expect(document.body.textContent).toContain('TODO tem 1 cartão: mover para');
    const select = document.querySelector(
      'select[aria-label="Destino dos cartões de TODO"]',
    ) as HTMLSelectElement;
    await act(async () => {
      select.value = 'done';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    expect(save.disabled).toBe(false);
    await act(async () => save.click());
    expect(api.saveColumns).toHaveBeenCalledWith(
      't1',
      expect.arrayContaining([expect.objectContaining({ slug: 'done' })]),
      [['todo', 'done']],
    );
  });
});
