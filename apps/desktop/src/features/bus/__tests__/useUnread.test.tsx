import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/types/generated/Agent';
import type { BusMessageEvent } from '@/types/generated/BusMessageEvent';

const unread = vi.fn();
let emit: (event: BusMessageEvent) => void = () => {};
vi.mock('@/features/bus/api', () => ({
  busApi: { unread: (...a: unknown[]) => unread(...a) },
  onBusMessage: (handler: (event: BusMessageEvent) => void) => {
    emit = handler;
    return Promise.resolve(() => {});
  },
  onBusRead: () => Promise.resolve(() => {}),
}));

const { useUnread } = await import('../useUnread');

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const agent = (id: string, handle: string, color: Agent['color']) =>
  ({ id, handle, color }) as Agent;

describe('useUnread', () => {
  it('conta as não lidas e pinta o badge com a cor de quem mandou', async () => {
    const agents = [agent('a1', 'backend', 'indigo'), agent('a2', 'frontend', 'emerald')];
    unread.mockResolvedValue([]);
    const hook: { current?: ReturnType<typeof useUnread> } = {};
    function Probe() {
      hook.current = useUnread('t1', agents);
      return null;
    }
    const root = createRoot(document.createElement('div'));
    await act(async () => root.render(<Probe />));
    expect(hook.current?.pendingOf('a2')).toBeUndefined();

    unread.mockResolvedValue([{ agentId: 'a2', count: 3 }]);
    await act(async () =>
      emit({
        teamId: 't1',
        recipients: 1,
        message: {
          id: 'm1',
          kind: 'message',
          from: '@backend',
          to: '@frontend',
          subject: null,
          body: 'oi',
          replyTo: null,
          meta: { priority: 'normal', attachments: [] },
          createdAt: 1,
          receipts: { recipients: 1, delivered: 0, read: 0 },
        },
      }),
    );
    expect(hook.current?.pendingOf('a2')).toEqual({ count: 3, color: 'indigo' });
    // Mensagem de outra equipe não relê.
    unread.mockClear();
    await act(async () =>
      emit({
        teamId: 'outra',
        recipients: 1,
        message: {
          id: 'm2',
          kind: 'message',
          from: '@x',
          to: '@y',
          subject: null,
          body: '',
          replyTo: null,
          meta: { priority: 'normal', attachments: [] },
          createdAt: 1,
          receipts: { recipients: 1, delivered: 0, read: 0 },
        },
      }),
    );
    expect(unread).not.toHaveBeenCalled();
    act(() => root.unmount());
  });
});
