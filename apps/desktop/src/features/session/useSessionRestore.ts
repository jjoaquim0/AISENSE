import { useEffect, useState } from 'react';
import { settingsApi } from '@/features/settings/api';
import { useSettings } from '@/features/settings/store';
import { useTeams } from '@/features/teams/store';
import { teamToRestore } from './restore';

/**
 * Persistência de sessão no front (F08-06): lembra a equipe aberta e, ao subir, reabre a
 * última. Vista, layout da grade, agente em foco e rolagem da linha do tempo vêm do
 * `teams.layout` (`useTeamLayout`); larguras da sidebar e do inspetor, do `usePanels`.
 */
export function useSessionRestore(): void {
  const session = useSettings((s) => s.view?.settings.session ?? null);
  const teams = useTeams((s) => s.teams);
  const selected = useTeams((s) => s.selectedTeamId);
  const selectTeam = useTeams((s) => s.selectTeam);
  const [restored, setRestored] = useState(false);

  // Uma vez, quando preferências e equipes chegaram.
  useEffect(() => {
    if (restored || !session || !teams) return;
    setRestored(true);
    const target = teamToRestore(session, teams, selected);
    if (target) selectTeam(target);
  }, [restored, session, teams, selected, selectTeam]);

  // Toda troca de equipe (inclusive voltar para a lista) vira a "última aberta". Antes de
  // restaurar, não: gravaria `null` por cima da equipe que ainda vai ser reaberta.
  useEffect(() => {
    if (!restored) return;
    void settingsApi.lastTeam(selected).catch(() => {});
  }, [restored, selected]);
}
