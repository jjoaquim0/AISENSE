import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentId } from '@/types/generated/AgentId';
import type { AppSettings } from '@/types/generated/AppSettings';
import type { Calibration } from '@/types/generated/Calibration';
import type { SettingsView } from '@/types/generated/SettingsView';
import type { StateRules } from '@/types/generated/StateRules';
import type { TeamId } from '@/types/generated/TeamId';

/** Único ponto do front que chama os comandos de Configurações (docs/10). */
export const settingsApi = {
  get: (): Promise<SettingsView> => invoke('settings_get'),
  /** Segredos e a última equipe não vêm do formulário: o core mantém os que tem. */
  save: (next: AppSettings): Promise<AppSettings> => invoke('settings_save', { next }),
  reset: (): Promise<AppSettings> => invoke('settings_reset'),
  onboardingDone: (): Promise<AppSettings> => invoke('settings_onboarding_done'),
  lastTeam: (teamId: TeamId | null): Promise<void> => invoke('settings_last_team', { teamId }),
  /** O valor vai direto para o keychain do SO; nunca volta para a interface. */
  setSecret: (adapterId: string, envName: string, value: string): Promise<AppSettings> =>
    invoke('secret_set', { adapterId, envName, value }),
  deleteSecret: (adapterId: string, envName: string): Promise<AppSettings> =>
    invoke('secret_delete', { adapterId, envName }),
  /** A tela atual do agente, sem ANSI (o que os regex enxergam). */
  calibrationScreen: (agentId: AgentId): Promise<string> =>
    invoke('calibration_screen', { agentId }),
  calibrationTest: (rules: StateRules, screen: string): Promise<Calibration> =>
    invoke('calibration_test', { rules, screen }),
  /** Grava no adaptador e aplica às sessões vivas. Devolve o arquivo gravado. */
  calibrationApply: (adapterId: string, rules: StateRules): Promise<string> =>
    invoke('calibration_apply', { adapterId, rules }),
  exportDiagnostics: (): Promise<string> => invoke('diagnostics_export'),
};

export function onSettingsChanged(handler: (settings: AppSettings) => void): Promise<UnlistenFn> {
  return listen<AppSettings>('settings:changed', ({ payload }) => handler(payload));
}
