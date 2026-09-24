import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/types/generated/Agent';
import type { BusMessageEvent } from '@/types/generated/BusMessageEvent';
import type { MessageView } from '@/types/generated/MessageView';

const timeline = vi.fn();
const send = vi.fn();
let emit: (event: BusMessageEvent) => void = () => {};
vi.mock('@/features/bus/api', () => ({
  busApi: {
    timeline: (...a: unknown[]) => timeline(...a),
    send: (...a: unknown[]) => send(...a),
    resume: vi.fn(),
  },
  onBusMessage: (handler: (event: BusMessageEvent) => void) => {
    emit = handler;
    return Promise.resolve(() => {});
  },
  onBusRead: () => Promise.resolve(() => {}),
}));

const { TimelineView } = await import('../TimelineView');
const model = await import('../timelineModel');

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

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
  receipts: { recipients: 1, delivered: 0, read: 0 },
  ...over,
});

describe('regras da linha do tempo', () => {
  it('junta sem repetir, na ordem do id, e a versão nova vence', () => {
    const merged = model.mergeMessages(
      [msg('m2'), msg('m1')],
      [msg('m3'), msg('m1', { receipts: { recipients: 1, delivered: 1, read: 1 } })],
    );
    expect(merged.map((m) => m.id)).toEqual(['m1', 'm2', 'm3']);
    expect(merged[0]?.receipts.read).toBe(1);
  });

  it('pergunta pendente conta o tempo; respondida ou expirada, não', () => {
    const ask = msg('q', {
      kind: 'request',
      meta: { priority: 'normal', attachments: [], timeoutS: 60 },
    });
    expect(model.askRemaining(ask, [ask], 1_000 + 15_000)).toBe(45);
    expect(model.askRemaining(ask, [ask, msg('r', { replyTo: 'q' })], 1_000)).toBeNull();
    expect(model.askRemaining(ask, [ask], 1_000 + 61_000)).toBeNull();
    expect(model.formatCountdown(125)).toBe('2:05');
  });

  it('filtros, recibos e destinos do compositor', () => {
    const all = [
      msg('a'),
      msg('b', { from: '@revisor', to: '#deploys' }),
      msg('c', { kind: 'system', from: 'aisense', to: '@voce' }),
    ];
    expect(model.applyFilter(all, { agent: '@revisor', onlyConversation: false })).toHaveLength(1);
    expect(model.applyFilter(all, { agent: null, onlyConversation: true })).toHaveLength(2);
    expect(
      model.receiptLabel(msg('x', { receipts: { recipients: 3, delivered: 2, read: 1 } })),
    ).toBe('lida por 1 de 3');
    expect(
      model.receiptLabel(msg('y', { receipts: { recipients: 0, delivered: 0, read: 0 } })),
    ).toBe(null);
    expect(model.destinations(['backend'], all)).toEqual(['@all', '@backend', '#deploys']);
    expect(model.nearBottom({ scrollTop: 900, scrollHeight: 1000, clientHeight: 80 })).toBe(true);
    expect(model.nearBottom({ scrollTop: 100, scrollHeight: 1000, clientHeight: 80 })).toBe(false);
  });
});

describe('janela virtual', () => {
  it('posições pelas alturas medidas e busca do índice visível', async () => {
    const { offsetsOf, indexAt, ESTIMATED_ROW } = await import('../useVirtualList');
    const offsets = offsetsOf(['a', 'b', 'c'], new Map([['b', 100]]));
    expect(offsets).toEqual([0, ESTIMATED_ROW, ESTIMATED_ROW + 100, 2 * ESTIMATED_ROW + 100]);
    expect(indexAt(offsets, 0)).toBe(0);
    expect(indexAt(offsets, ESTIMATED_ROW + 50)).toBe(1);
    expect(indexAt(offsets, 10_000)).toBe(2);
    // 10 mil itens: a busca continua instantânea.
    const many = offsetsOf(
      Array.from({ length: 10_000 }, (_, i) => `k${i}`),
      new Map(),
    );
    expect(indexAt(many, 5_000 * ESTIMATED_ROW)).toBe(5_000);
  });
});

describe('<TimelineView />', () => {
  let container: HTMLDivElement;
  let root: Root;
  const agents = [{ id: 'a1', handle: 'backend', color: 'indigo' } as Agent];

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  it('mostra a conversa, recebe mensagem ao vivo e manda como @voce', async () => {
    // A API devolve a mais nova primeiro.
    timeline.mockResolvedValue([msg('m2', { body: 'segunda' }), msg('m1', { body: 'primeira' })]);
    send.mockResolvedValue(['m9']);
    await act(async () => root.render(<TimelineView teamId="t1" agents={agents} />));
    const bodies = () => [...container.querySelectorAll('li p')].map((p) => p.textContent);
    expect(bodies()).toEqual(['primeira', 'segunda']);
    expect(container.querySelector('li > div')?.getAttribute('style')).toContain('--agent-indigo');

    await act(async () =>
      emit({ teamId: 't1', recipients: 1, message: msg('m3', { body: 'ao vivo' }) }),
    );
    await act(async () =>
      emit({ teamId: 'outra', recipients: 1, message: msg('m4', { body: 'de fora' }) }),
    );
    expect(bodies()).toEqual(['primeira', 'segunda', 'ao vivo']);

    const textarea = container.querySelector('textarea') as HTMLTextAreaElement;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(
        textarea,
        'parem e resumam',
      );
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await act(async () =>
      textarea.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })),
    );
    expect(send).toHaveBeenCalledWith('t1', ['@all'], 'parem e resumam');
  });
});
