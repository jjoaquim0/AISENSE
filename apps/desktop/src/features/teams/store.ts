import { create } from 'zustand';
import type { TeamId } from '@/types/generated/TeamId';
import type { TeamSummary } from '@/types/generated/TeamSummary';
import { errorMessage, teamsApi } from './api';

interface TeamsState {
  teams: TeamSummary[] | null;
  showArchived: boolean;
  error: string | null;
  /** Assistente de criação (T3) aberto — o `+` do trilho e o botão da T2 abrem o mesmo. */
  wizardOpen: boolean;
  /** Equipe aberta; `null` mostra a lista (T2). */
  selectedTeamId: TeamId | null;
  load: () => Promise<void>;
  selectTeam: (teamId: TeamId | null) => void;
  setShowArchived: (show: boolean) => void;
  setWizardOpen: (open: boolean) => void;
}

export const useTeams = create<TeamsState>((set, get) => ({
  teams: null,
  showArchived: false,
  error: null,
  wizardOpen: false,
  selectedTeamId: null,
  load: async () => {
    try {
      set({ teams: await teamsApi.list(get().showArchived), error: null });
    } catch (e: unknown) {
      set({ error: errorMessage(e) });
    }
  },
  setShowArchived: (showArchived) => {
    set({ showArchived });
    void get().load();
  },
  setWizardOpen: (wizardOpen) => set({ wizardOpen }),
  selectTeam: (selectedTeamId) => set({ selectedTeamId }),
}));
