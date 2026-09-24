import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Agent } from '@/types/generated/Agent';
import type { AgentBootChanged } from '@/types/generated/AgentBootChanged';
import type { AgentDraft } from '@/types/generated/AgentDraft';
import type { AgentId } from '@/types/generated/AgentId';
import type { AgentPreview } from '@/types/generated/AgentPreview';
import type { AgentState } from '@/types/generated/AgentState';
import type { AgentStateChanged } from '@/types/generated/AgentStateChanged';
import type { AgentUpdate } from '@/types/generated/AgentUpdate';
import type { BootDelivery } from '@/types/generated/BootDelivery';
import type { SessionId } from '@/types/generated/SessionId';
import type { SessionSummary } from '@/types/generated/SessionSummary';
import type { StartOutcome } from '@/types/generated/StartOutcome';
import type { TeamId } from '@/types/generated/TeamId';
import type { Transcript } from '@/types/generated/Transcript';

/** Único ponto do front que chama os comandos de agente (docs/10). */
export const agentsApi = {
  list: (teamId: TeamId): Promise<Agent[]> => invoke('agents_list', { teamId }),
  create: (teamId: TeamId, draft: AgentDraft): Promise<Agent> =>
    invoke('agent_create', { teamId, draft }),
  /** `restartRequired` diz se a mudança só vale depois de reiniciar o agente. */
  update: (agentId: AgentId, draft: AgentDraft): Promise<AgentUpdate> =>
    invoke('agent_update', { agentId, draft }),
  /** Últimas linhas da tela de cada agente (miniaturas da vista Foco). */
  previews: (agentIds: AgentId[], lines: number): Promise<AgentPreview[]> =>
    invoke('agent_previews', { agentIds, lines }),
  /** Mesma configuração, handle e cor livres (menu ⋮ do painel). */
  duplicate: (agentId: AgentId): Promise<Agent> => invoke('agent_duplicate', { agentId }),
  /** Nova ordem da equipe inteira (sidebar); devolve a lista já reordenada. */
  reorder: (teamId: TeamId, order: AgentId[]): Promise<Agent[]> =>
    invoke('agents_reorder', { teamId, order }),
  remove: (agentId: AgentId): Promise<void> => invoke('agent_delete', { agentId }),
  suggestHandle: (name: string): Promise<string | null> => invoke('handle_suggest', { name }),

  /** Devolve onde o agente foi trabalhar e alguma ressalva (sem bancada, setup que falhou). */
  start: (agentId: AgentId): Promise<StartOutcome> => invoke('agent_start', { agentId }),
  stop: (agentId: AgentId): Promise<void> => invoke('agent_stop', { agentId }),
  restart: (agentId: AgentId): Promise<StartOutcome> => invoke('agent_restart', { agentId }),
  state: (agentId: AgentId): Promise<AgentState> => invoke('agent_state', { agentId }),
  /** Por onde o `BOOT.md` da sessão atual foi entregue (F04-06); `null` se não subiu. */
  boot: (agentId: AgentId): Promise<BootDelivery | null> => invoke('agent_boot', { agentId }),

  /** Sessões do agente, mais recente primeiro (aba Logs do inspetor). */
  sessions: (agentId: AgentId): Promise<SessionSummary[]> => invoke('agent_sessions', { agentId }),
  /** O final da transcrição de uma sessão, já em texto. */
  transcript: (agentId: AgentId, sessionId: SessionId): Promise<Transcript> =>
    invoke('session_transcript', { agentId, sessionId }),
  /** Grava a transcrição inteira em `path`; devolve os bytes escritos. */
  exportTranscript: (agentId: AgentId, sessionId: SessionId, path: string): Promise<number> =>
    invoke('session_export', { agentId, sessionId, path }),
};

/** Toda mudança de estado de qualquer agente (inclusive reinícios da política). */
export function onAgentState(handler: (event: AgentStateChanged) => void): Promise<UnlistenFn> {
  return listen<AgentStateChanged>('agent:state', ({ payload }) => handler(payload));
}

/** A entrega do `BOOT.md` mudou — pelo terminal, ela termina depois do start. */
export function onAgentBoot(handler: (event: AgentBootChanged) => void): Promise<UnlistenFn> {
  return listen<AgentBootChanged>('agent:boot', ({ payload }) => handler(payload));
}
