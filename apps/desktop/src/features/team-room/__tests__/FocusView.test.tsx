import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/types/generated/Agent';

const previews = vi.fn();
vi.mock('@/features/agents/api', () => ({
  agentsApi: { previews: (...args: unknown[]) => previews(...args) },
}));

const { FocusView } = await import('../components/FocusView');

// React 19: `act` fora de uma biblioteca de testes precisa desta marcação.
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const agent = (id: string, handle: string): Agent => ({
  id,
  teamId: 'tem_1',
  handle,
  name: handle,
  role: '',
  adapterId: 'shell',
  model: null,
  workdir: null,
  env: {},
  args: [],
  color: 'violet',
  autostart: false,
  restartPolicy: 'on-crash',
  deliveryMode: 'pull',
  autonomy: 'ask',
  workbench: 'inherit',
  position: 0,
  createdAt: 0,
  updatedAt: 0,
});

describe('vista Foco', () => {
  let host: HTMLDivElement;
  beforeEach(() => {
    vi.useFakeTimers();
    host = document.createElement('div');
    document.body.append(host);
    previews.mockImplementation((ids: string[]) =>
      Promise.resolve(ids.map((agentId) => ({ agentId, lines: [`saída de ${agentId}`, '$'] }))),
    );
  });
  afterEach(() => {
    vi.useRealTimers();
    host.remove();
    previews.mockReset();
  });

  const render = async (focusedId: string | null, onFocus = vi.fn()) => {
    const root = createRoot(host);
    await act(async () => {
      root.render(
        <FocusView
          order={['a', 'b', 'c']}
          agents={[agent('a', 'arquiteto'), agent('b', 'backend'), agent('c', 'revisor')]}
          focusedId={focusedId}
          stateOf={() => 'idle'}
          confidenceOf={() => undefined}
          renderPane={(id) => <div data-testid="big">{id}</div>}
          onFocus={onFocus}
          onAdd={() => {}}
        />,
      );
    });
    return root;
  };

  it('mostra o agente em foco grande e os outros como miniaturas de texto, sem xterm', async () => {
    const root = await render('b');
    expect(host.querySelector('[data-testid=big]')?.textContent).toBe('b');
    const thumbs = [...host.querySelectorAll('nav button[aria-label^="Focar"]')];
    expect(thumbs.map((t) => t.getAttribute('aria-label'))).toEqual([
      'Focar @arquiteto',
      'Focar @revisor',
    ]);
    expect(host.textContent).toContain('saída de a');
    expect(host.textContent).toContain('saída de c');
    expect(host.querySelector('.xterm, canvas')).toBeNull();
    act(() => root.unmount());
  });

  it('busca as prévias de todos numa chamada só, a 2 fps', async () => {
    const root = await render('a');
    expect(previews).toHaveBeenCalledTimes(1);
    expect(previews).toHaveBeenLastCalledWith(['b', 'c'], 4);
    await act(async () => {
      vi.advanceTimersByTime(1_000);
    });
    expect(previews).toHaveBeenCalledTimes(3);
    act(() => root.unmount());
    await act(async () => {
      vi.advanceTimersByTime(2_000);
    });
    expect(previews).toHaveBeenCalledTimes(3);
  });

  it('clicar numa miniatura foca aquele agente', async () => {
    const onFocus = vi.fn();
    const root = await render('a', onFocus);
    const revisor = host.querySelector<HTMLButtonElement>('button[aria-label="Focar @revisor"]');
    act(() => revisor?.click());
    expect(onFocus).toHaveBeenCalledWith('c');
    act(() => root.unmount());
  });
});
