import { invoke } from '@tauri-apps/api/core';
import type { TeamId } from '@/types/generated/TeamId';

/** Único ponto do front que conta ao core o que está na tela (docs/10). */
export const sessionApi = {
  /**
   * Equipe aberta na Sala da Equipe agora; `null` em qualquer outra tela. O core não
   * notifica pelo SO sobre a equipe que você está olhando com a janela em foco (F08-07).
   */
  viewing: (teamId: TeamId | null): Promise<void> => invoke('ui_viewing', { teamId }),
};
