import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentId } from '@/types/generated/AgentId';
import type { AgentState } from '@/types/generated/AgentState';
import type { AgentStateChanged } from '@/types/generated/AgentStateChanged';

/** Único ponto do front que chama os comandos de ciclo de vida do agente (docs/10). */
export const agentsApi = {
  start: (agentId: AgentId): Promise<void> => invoke('agent_start', { agentId }),
  stop: (agentId: AgentId): Promise<void> => invoke('agent_stop', { agentId }),
  restart: (agentId: AgentId): Promise<void> => invoke('agent_restart', { agentId }),
  state: (agentId: AgentId): Promise<AgentState> => invoke('agent_state', { agentId }),
};

/** Toda mudança de estado de qualquer agente (inclusive reinícios da política). */
export function onAgentState(handler: (event: AgentStateChanged) => void): Promise<UnlistenFn> {
  return listen<AgentStateChanged>('agent:state', ({ payload }) => handler(payload));
}
