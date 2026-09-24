import type { SessionSettings } from '@/types/generated/SessionSettings';
import type { TeamId } from '@/types/generated/TeamId';
import type { TeamSummary } from '@/types/generated/TeamSummary';

/**
 * Qual equipe reabrir ao subir o app (F08-06): a última aberta, se a opção está ligada e
 * ela ainda existe sem estar arquivada. Nada muda se o usuário já abriu outra.
 */
export function teamToRestore(
  session: SessionSettings,
  teams: TeamSummary[],
  selected: TeamId | null,
): TeamId | null {
  if (selected || !session.restoreLastTeam || !session.lastTeam) return null;
  const team = teams.find((t) => t.team.id === session.lastTeam);
  return team && !team.team.archivedAt ? team.team.id : null;
}
