import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui';
import type { Note } from '@/types/generated/Note';

const list = vi.fn();
const read = vi.fn();
const save = vi.fn();
vi.mock('@/features/notes/api', () => ({
  notesApi: {
    list: (...a: unknown[]) => list(...a),
    read: (...a: unknown[]) => read(...a),
    save: (...a: unknown[]) => save(...a),
    search: () => Promise.resolve([]),
    create: vi.fn(),
    remove: vi.fn(),
  },
}));

const { NotesPanel, NOTES_POLL_MS } = await import('../NotesPanel');
const { isValidSlug, slugify } = await import('../notesModel');

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('nomes de nota', () => {
  it('slug a partir do título, no formato que o core aceita', () => {
    expect(slugify('Decisões técnicas')).toBe('decisoes-tecnicas');
    expect(slugify('  API v2 / Auth!  ')).toBe('api-v2-auth');
    expect(isValidSlug('decisoes-tecnicas')).toBe(true);
    expect(isValidSlug('../fora')).toBe(false);
    expect(isValidSlug('-x')).toBe(false);
  });
});

describe('<NotesPanel />', () => {
  let container: HTMLDivElement;
  let root: Root;
  const note = (content: string, hash: string): Note => ({
    slug: 'decisoes',
    title: 'Decisões',
    content,
    hash,
  });

  beforeEach(() => {
    vi.useFakeTimers();
    list.mockResolvedValue([{ slug: 'decisoes', title: 'Decisões', updatedAt: 0, bytes: 10 }]);
    read.mockResolvedValue(note('# Decisões\nusar SQLite\n', 'h1'));
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  const render = async () => {
    await act(async () => {
      root.render(
        <TooltipProvider>
          <NotesPanel teamId="t1" teamName="Squad" onClose={() => {}} />
        </TooltipProvider>,
      );
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
  };

  const type = async (value: string) => {
    const textarea = container.querySelector('textarea') as HTMLTextAreaElement;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(
        textarea,
        value,
      );
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
    });
  };

  const button = (label: string) =>
    [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(label)) as
      | HTMLButtonElement
      | undefined;

  it('abre a mais recente e salva com o hash lido', async () => {
    await render();
    expect(read).toHaveBeenCalledWith('t1', 'decisoes');
    expect(container.querySelector('textarea')?.value).toBe('# Decisões\nusar SQLite\n');
    save.mockResolvedValue({ kind: 'saved', note: note('# Decisões\nusar WAL\n', 'h2') });
    await type('# Decisões\nusar WAL\n');
    await act(async () => button('Salvar')?.click());
    expect(save).toHaveBeenCalledWith('t1', 'decisoes', '# Decisões\nusar WAL\n', 'h1');
    expect(container.textContent).toContain('Salvo.');
  });

  it('hash desatualizado mostra o diff e deixa gravar por cima com o hash atual', async () => {
    await render();
    save.mockResolvedValueOnce({
      kind: 'stale',
      currentHash: 'h9',
      diff: [
        { kind: 'same', text: '# Decisões' },
        { kind: 'removed', text: 'usar SQLite' },
        { kind: 'added', text: 'usar Postgres' },
      ],
    });
    await type('# Decisões\nusar Postgres\n');
    await act(async () => button('Salvar')?.click());
    const dialog = document.querySelector('[role="dialog"][aria-labelledby]')?.textContent ?? '';
    expect(dialog).toContain('A nota mudou desde que você abriu');
    expect(dialog).toContain('− usar SQLite');
    expect(dialog).toContain('+ usar Postgres');

    save.mockResolvedValueOnce({ kind: 'saved', note: note('# Decisões\nusar Postgres\n', 'h10') });
    await act(async () => button('Gravar a minha por cima')?.click());
    expect(save).toHaveBeenLastCalledWith('t1', 'decisoes', '# Decisões\nusar Postgres\n', 'h9');
  });

  it('um agente mudando a nota: sem edição acompanha, com edição avisa', async () => {
    await render();
    read.mockResolvedValue(note('# Decisões\nusar SQLite\nusar WAL\n', 'h2'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(NOTES_POLL_MS);
    });
    expect(container.querySelector('textarea')?.value).toContain('usar WAL');

    await type('minha edição');
    read.mockResolvedValue(note('outra coisa', 'h3'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(NOTES_POLL_MS);
    });
    expect(container.querySelector('textarea')?.value).toBe('minha edição');
    expect(container.textContent).toContain('Um agente mudou esta nota enquanto você editava');
  });
});
