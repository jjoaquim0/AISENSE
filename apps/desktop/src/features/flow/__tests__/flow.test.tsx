import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/types/generated/Agent';
import type { BoardEvent } from '@/types/generated/BoardEvent';
import type { MessageView } from '@/types/generated/MessageView';
import {
  aggregateEdges,
  autoLayout,
  lastSaid,
  mergePositions,
  readFlowPositions,
  strokeWidth,
} from '../flowModel';

const msg = (id: string, over: Partial<MessageView> = {}): MessageView => ({
  id,
  kind: 'message',
  from: '@backend',
  to: '@frontend',
  subject: null,
  body: `corpo ${id}`,
  replyTo: null,
  meta: { priority: 'normal', attachments: [] },
  createdAt: 1_000,
  receipts: { recipients: 1, delivered: 1, read: 0, failed: 0 },
  ...over,
});

describe('modelo do Fluxo', () => {
  it('agrega por par, conta, marca pergunta pendente e falha', () => {
    const now = 10_000;
    const edges = aggregateEdges(
      [
        msg('m1'),
        msg('m2', { body: 'contrato v2\nresto', createdAt: 2_000 }),
        msg('m3', {
          kind: 'request',
          from: '@backend',
          to: '@revisor',
          meta: { priority: 'normal', attachments: [], timeoutS: 60 },
        }),
        msg('m4', { from: '@frontend', to: '#geral' }),
        msg('m5', { from: '@voce', to: '@backend' }),
        msg('m6', {
          from: '@revisor',
          to: '@backend',
          receipts: { recipients: 1, delivered: 0, read: 0, failed: 1 },
        }),
        msg('velha', { createdAt: -1_000_000 }),
      ],
      ['backend', 'frontend', 'revisor'],
      60_000,
      now,
    );
    const byId = new Map(edges.map((e) => [e.id, e]));
    expect(byId.get('@backend->@frontend')).toMatchObject({ count: 2, preview: 'contrato v2' });
    expect(byId.get('@backend->@revisor')?.askRemaining).toBe(51);
    expect(byId.has('@frontend->#geral')).toBe(true);
    expect(byId.has('@voce->@backend')).toBe(false);
    expect(byId.get('@revisor->@backend')?.failed).toBe(true);
    expect(strokeWidth(1)).toBe(1.5);
    expect(strokeWidth(1000)).toBe(6);
  });

  it('layout em camadas, posições salvas vencem, leitura tolerante', () => {
    const layout = autoLayout(
      ['@a', '@b', '@c', '#geral'],
      [
        { from: '@a', to: '@b' },
        { from: '@b', to: '@c' },
      ],
    );
    expect(layout.get('@a')?.y).toBeLessThan(layout.get('@b')?.y ?? 0);
    expect(layout.get('@b')?.y).toBeLessThan(layout.get('@c')?.y ?? 0);
    expect(layout.get('#geral')?.x).toBeGreaterThan(layout.get('@a')?.x ?? 0);
    const merged = mergePositions(layout, { '@a': { x: 9, y: 9 }, '@sumiu': { x: 1, y: 1 } });
    expect(merged.get('@a')).toEqual({ x: 9, y: 9 });
    expect(merged.has('@sumiu')).toBe(false);
    expect(readFlowPositions({ flow: { positions: { '@a': { x: 1, y: 2 }, '@b': 'x' } } })).toEqual(
      {
        '@a': { x: 1, y: 2 },
      },
    );
    expect(readFlowPositions(null)).toEqual({});
    expect(lastSaid([msg('m1'), msg('m2', { body: 'último' })], 'backend')?.body).toBe('último');
  });
});

// ─── componente: o ReactFlow é trocado por um espião que guarda nós e arestas ───

const flowProps: { nodes: unknown[]; edges: unknown[] }[] = [];
vi.mock('@xyflow/react', () => ({
  ReactFlow: (props: { nodes: unknown[]; edges: unknown[] }) => {
    flowProps.push(props);
    return null;
  },
  Background: () => null,
  Handle: () => null,
  Position: { Top: 'top', Bottom: 'bottom', Left: 'left', Right: 'right' },
}));
vi.mock('@xyflow/react/dist/style.css', () => ({}));
const getBoard = vi.fn();
let boardChanged: (e: BoardEvent) => void = () => {};
vi.mock('@/features/board/api', () => ({
  boardApi: { get: (...a: unknown[]) => getBoard(...a) },
  onBoardChanged: (h: (e: BoardEvent) => void) => {
    boardChanged = h;
    return Promise.resolve(() => {});
  },
}));
vi.mock('@/features/bus/api', () => ({
  busApi: {
    timeline: () => Promise.resolve([msg('m1', { createdAt: Date.now() })]),
    channels: () => Promise.resolve([]),
  },
  onBusMessage: () => Promise.resolve(() => {}),
}));

const { FlowView } = await import('../FlowView');
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('<FlowView />', () => {
  it('mostra agentes, mensagens e o cartão em andamento, e acompanha o quadro', async () => {
    const agents = [
      { id: 'a1', handle: 'backend', color: 'indigo' },
      { id: 'a2', handle: 'frontend', color: 'emerald' },
    ] as Agent[];
    const board = (column: string) => ({
      teamId: 't1',
      teamName: 'Squad',
      board: { id: 'b', teamId: 't1', automations: [], createdAt: 0 },
      columns: [
        { id: 'todo', kind: 'ready' },
        { id: 'doing', kind: 'active' },
      ],
      cards: [
        {
          id: 'tsk_01ABCDEF',
          title: 'Refresh token',
          assignee: 'a1',
          columnId: column,
          columnSince: 1,
        },
      ],
      agents: [],
      now: 0,
    });
    getBoard.mockResolvedValueOnce(board('doing')).mockResolvedValue(board('todo'));
    const root = createRoot(document.createElement('div'));
    await act(async () =>
      root.render(
        <FlowView
          teamId="t1"
          agents={agents}
          stateOf={() => 'idle'}
          positions={{}}
          onPositions={() => {}}
          onOpenAgent={() => {}}
        />,
      ),
    );
    const last = () =>
      flowProps[flowProps.length - 1] as { nodes: { id: string }[]; edges: { id: string }[] };
    expect(last().nodes.map((n) => n.id)).toEqual(['@backend', 'card:tsk_01ABCDEF', '@frontend']);
    expect(last().edges.map((e) => e.id)).toEqual([
      '@backend->@frontend',
      'card-edge:tsk_01ABCDEF',
    ]);
    // Cartão saiu de Fazendo no quadro: o canvas relê e o nó some.
    const started = performance.now();
    await act(async () =>
      boardChanged({
        teamId: 't1',
        cardId: 'tsk_01ABCDEF',
        action: 'moved',
        actor: { kind: 'human' },
        involved: [],
        at: 0,
      }),
    );
    expect(last().nodes.map((n) => n.id)).toEqual(['@backend', '@frontend']);
    expect(performance.now() - started).toBeLessThan(300);
    act(() => root.unmount());
  });
});
