import type { AgentState } from '@/types/generated/AgentState';

export type PaneAction = 'restart' | 'stop' | 'start' | 'clear' | 'duplicate' | 'configure';

export interface PaneMenuItem {
  action: PaneAction;
  label: string;
  /** Item presente mas indisponível no estado atual (ex.: limpar sem histórico). */
  disabled: boolean;
  danger?: boolean;
}

const RUNNING: AgentState[] = ['starting', 'idle', 'busy', 'awaiting_input'];

export function isRunning(state: AgentState): boolean {
  return RUNNING.includes(state);
}

/**
 * Itens do menu `⋮` do painel (docs/09, T4.1). Os itens não somem conforme o estado —
 * posição fixa é memória muscular; o que não se aplica fica desabilitado. A exceção é
 * Iniciar/Parar, que trocam de lugar: são a mesma ação vista de dois lados.
 */
export function paneMenu(state: AgentState): PaneMenuItem[] {
  const running = isRunning(state);
  const hasHistory = running || state === 'failed';
  return [
    { action: 'restart', label: 'Reiniciar', disabled: !running },
    running
      ? { action: 'stop', label: 'Parar', disabled: false, danger: true }
      : { action: 'start', label: 'Iniciar', disabled: false },
    { action: 'clear', label: 'Limpar terminal', disabled: !hasHistory },
    { action: 'duplicate', label: 'Duplicar agente', disabled: false },
    { action: 'configure', label: 'Configurar…', disabled: false },
  ];
}
