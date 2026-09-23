import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui';
import type { Agent } from '@/types/generated/Agent';
import type { AgentStateChanged } from '@/types/generated/AgentStateChanged';
import type { AgentSummary } from '@/types/generated/AgentSummary';

let emit: (event: AgentStateChanged) => void = () => {};
vi.mock('@/features/agents/api', () => ({
  onAgentState: (handler: (event: AgentStateChanged) => void) => {
    emit = handler;
    return Promise.resolve(() => {});
  },
}));

const { AgentSidebar } = await import('../components/AgentSidebar');
const { useLiveStates } = await import('../hooks/useLiveStates');
const { announcement } = await import('../sidebar');
const { ShellSlot, useShellSlots } = await import('@/features/shell/slots');

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

const AGENTS = [agent('a', 'arquiteto'), agent('b', 'backend')];
const summary = (id: string, state: AgentSummary['state']): AgentSummary => ({
  id,
  handle: id,
  name: id,
  color: 'violet',
  adapterId: 'shell',
  autostart: false,
  state,
});

type Handlers = Partial<Parameters<typeof AgentSidebar>[0]>;

/** A sidebar como a Sala da Equipe a usa: estados vindos de `useLiveStates`. */
function Harness({ handlers }: { handlers: Handlers }) {
  const { stateOf, confidenceOf } = useLiveStates([summary('a', 'idle'), summary('b', 'stopped')]);
  return (
    <TooltipProvider>
      <AgentSidebar
        agents={AGENTS}
        selectedId="a"
        stateOf={stateOf}
        confidenceOf={confidenceOf}
        onSelect={() => {}}
        onInspect={() => {}}
        onReorder={() => {}}
        onStart={() => {}}
        onStop={() => {}}
        onEdit={() => {}}
        onDelete={() => {}}
        onAdd={() => {}}
        {...handlers}
      />
    </TooltipProvider>
  );
}

describe('sidebar de agentes', () => {
  let host: HTMLDivElement;
  let root: Root;
  beforeEach(() => {
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
  });
  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  const render = async (handlers: Handlers = {}) => {
    await act(async () => root.render(<Harness handlers={handlers} />));
  };
  const row = (handle: string) =>
    [...host.querySelectorAll<HTMLButtonElement>('button[title*="focar"]')].find((b) =>
      b.textContent?.includes(`@${handle}`),
    );

  it('reflete a mudança de estado sem esperar o core, bem abaixo de 200 ms', async () => {
    await render();
    expect(row('backend')?.textContent).toContain('Parado');

    const began = performance.now();
    act(() => emit({ agentId: 'b', state: 'busy', confidence: 'high' }));
    const took = performance.now() - began;

    expect(row('backend')?.textContent).toContain('Trabalhando');
    expect(took).toBeLessThan(200);

    act(() => emit({ agentId: 'b', state: 'idle', confidence: 'low' }));
    expect(row('backend')?.textContent).toContain('Ocioso?');
  });

  it('clique foca, duplo clique abre o inspetor', async () => {
    const onSelect = vi.fn();
    const onInspect = vi.fn();
    await render({ onSelect, onInspect });
    const backend = row('backend');
    act(() => backend?.click());
    expect(onSelect).toHaveBeenCalledWith('b');
    act(() => {
      backend?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    });
    expect(onInspect).toHaveBeenCalledWith('b');
    expect(row('arquiteto')?.getAttribute('aria-current')).toBe('true');
  });

  it('mostra parar para quem está de pé e iniciar para quem está parado', async () => {
    const onStart = vi.fn();
    const onStop = vi.fn();
    await render({ onStart, onStop });
    act(() => host.querySelector<HTMLButtonElement>('[aria-label="Parar @arquiteto"]')?.click());
    act(() => host.querySelector<HTMLButtonElement>('[aria-label="Iniciar @backend"]')?.click());
    expect(onStop).toHaveBeenCalledWith('a');
    expect(onStart).toHaveBeenCalledWith('b');
  });

  it('tem alça de reordenar por agente, e o badge de mensagens só aparece com pendência', async () => {
    await render({
      pendingOf: (id) => (id === 'a' ? { count: 12, color: 'rose' } : undefined),
    });
    expect(host.querySelector('[aria-label="Reordenar @backend"]')).not.toBeNull();
    const badges = [...host.querySelectorAll('[aria-label$="pendentes"]')];
    expect(badges).toHaveLength(1);
    expect(badges[0]?.textContent).toBe('9+');
    expect(badges[0]?.getAttribute('aria-label')).toBe('12 mensagens pendentes');
  });

  it('anuncia a leitores de tela só o que pede atenção', async () => {
    await render();
    const live = () => host.querySelector('p[role=status]')?.textContent;
    act(() => emit({ agentId: 'a', state: 'busy', confidence: 'high' }));
    expect(live()).toBe('');
    act(() => emit({ agentId: 'a', state: 'awaiting_input', confidence: 'high' }));
    expect(live()).toBe('@arquiteto está aguardando você');
  });
});

describe('announcement', () => {
  it('fica em silêncio para trabalhando, ocioso e iniciando', () => {
    for (const state of ['busy', 'idle', 'starting'] as const) {
      expect(announcement('x', state)).toBeNull();
    }
    expect(announcement('x', 'failed')).toBe('@x caiu com erro');
  });
});

describe('ShellSlot', () => {
  it('renderiza na região do shell e a marca como ocupada enquanto montado', () => {
    const region = document.createElement('div');
    const host = document.createElement('div');
    document.body.append(region, host);
    act(() => useShellSlots.getState().setHost('sidebar', region));
    const root = createRoot(host);
    act(() =>
      root.render(
        <ShellSlot name="sidebar">
          <p>lista</p>
        </ShellSlot>,
      ),
    );
    expect(region.textContent).toBe('lista');
    expect(host.textContent).toBe('');
    expect(useShellSlots.getState().filled.sidebar).toBe(1);
    act(() => root.unmount());
    expect(region.textContent).toBe('');
    expect(useShellSlots.getState().filled.sidebar).toBe(0);
    region.remove();
    host.remove();
  });
});
