import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentId } from '@/types/generated/AgentId';
import type { AgentSkill } from '@/types/generated/AgentSkill';
import type { SkillLibraryView } from '@/types/generated/SkillLibraryView';
import type { SkillPlan } from '@/types/generated/SkillPlan';

/** Único ponto do front que chama os comandos de skills (docs/10). */
export const skillsApi = {
  library: (): Promise<SkillLibraryView> => invoke('skills_library'),
  ofAgent: (agentId: AgentId): Promise<AgentSkill[]> => invoke('agent_skills_get', { agentId }),
  /** Troca a lista inteira; a ordem é a de injeção. Vale no próximo início. */
  setForAgent: (agentId: AgentId, skills: AgentSkill[]): Promise<void> =>
    invoke('agent_skills_set', { agentId, skills }),
  /** O que o agente levaria se subisse agora: ativas em ordem e ignoradas com o porquê. */
  plan: (agentId: AgentId): Promise<SkillPlan> => invoke('agent_skills_plan', { agentId }),
};

/** A biblioteca mudou no disco (hot-reload): peça a lista de novo. */
export function onSkillsChanged(handler: () => void): Promise<UnlistenFn> {
  return listen('skills:changed', () => handler());
}
