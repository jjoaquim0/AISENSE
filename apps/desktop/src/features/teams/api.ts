import { invoke } from '@tauri-apps/api/core';
import type { AgentDraft } from '@/types/generated/AgentDraft';
import type { AgentStartFailure } from '@/types/generated/AgentStartFailure';
import type { PlannedAgent } from '@/types/generated/PlannedAgent';
import type { Team } from '@/types/generated/Team';
import type { TeamDraft } from '@/types/generated/TeamDraft';
import type { TeamId } from '@/types/generated/TeamId';
import type { TeamSummary } from '@/types/generated/TeamSummary';
import type { TeamTemplate } from '@/types/generated/TeamTemplate';

/** Único ponto do front que chama os comandos de equipe (docs/10). */
export const teamsApi = {
  list: (includeArchived = false): Promise<TeamSummary[]> =>
    invoke('teams_list', { includeArchived }),
  planTemplate: (template: TeamTemplate): Promise<PlannedAgent[]> =>
    invoke('team_template_plan', { template }),
  create: (team: TeamDraft, agents: AgentDraft[]): Promise<Team> =>
    invoke('team_create', { team, agents }),
  setArchived: (teamId: TeamId, archived: boolean): Promise<void> =>
    invoke('team_set_archived', { teamId, archived }),
  /** O core confere `confirmName` de novo: a interface não é a única barreira. */
  remove: (teamId: TeamId, confirmName: string): Promise<void> =>
    invoke('team_delete', { teamId, confirmName }),
  start: (teamId: TeamId): Promise<AgentStartFailure[]> => invoke('team_start', { teamId }),
};

/** Mensagem legível de um `CommandError` (ou de qualquer coisa que o `invoke` rejeite). */
export function errorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const hint = 'hint' in error && error.hint ? ` ${String(error.hint)}` : '';
    return `${String(error.message)}${hint}`;
  }
  return String(error);
}
