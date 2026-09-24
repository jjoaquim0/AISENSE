import type { AgentState } from '@/types/generated/AgentState';

export const STATE_LABEL: Record<AgentState, string> = {
  stopped: 'parado',
  starting: 'iniciando',
  idle: 'ocioso',
  busy: 'ocupado',
  awaiting_input: 'aguardando você',
  failed: 'falhou',
};

/** `idle_regex` → `idle`, e assim por diante. */
export function stateOfField(field: string): AgentState | null {
  switch (field) {
    case 'idle_regex':
      return 'idle';
    case 'busy_regex':
      return 'busy';
    case 'awaiting_regex':
      return 'awaiting_input';
    default:
      return null;
  }
}
