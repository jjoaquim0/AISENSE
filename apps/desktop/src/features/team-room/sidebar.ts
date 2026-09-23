import type { AgentState } from '@/types/generated/AgentState';

const RUNNING: readonly AgentState[] = ['starting', 'idle', 'busy', 'awaiting_input'];

/** Processo de pé (mesma regra de `AgentState::is_running` no core). */
export function isRunning(state: AgentState): boolean {
  return RUNNING.includes(state);
}

/**
 * O que a sidebar anuncia a leitores de tela quando um agente muda de estado. Só o que
 * pede atenção: trabalhando/ocioso alternam o tempo todo e virariam ruído.
 */
export function announcement(handle: string, state: AgentState): string | null {
  switch (state) {
    case 'awaiting_input':
      return `@${handle} está aguardando você`;
    case 'failed':
      return `@${handle} caiu com erro`;
    case 'stopped':
      return `@${handle} parou`;
    default:
      return null;
  }
}
