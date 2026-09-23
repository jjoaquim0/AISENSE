import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentId } from '@/types/generated/AgentId';
import type { AgentSkill } from '@/types/generated/AgentSkill';
import type { OpenedSkill } from '@/types/generated/OpenedSkill';
import type { Skill } from '@/types/generated/Skill';
import type { SkillCheck } from '@/types/generated/SkillCheck';
import type { SkillLibraryView } from '@/types/generated/SkillLibraryView';
import type { SkillPlan } from '@/types/generated/SkillPlan';
import type { SkillUser } from '@/types/generated/SkillUser';

/** Único ponto do front que chama os comandos de skills (docs/10). */
export const skillsApi = {
  library: (): Promise<SkillLibraryView> => invoke('skills_library'),
  ofAgent: (agentId: AgentId): Promise<AgentSkill[]> => invoke('agent_skills_get', { agentId }),
  /** Troca a lista inteira; a ordem é a de injeção. Vale no próximo início. */
  setForAgent: (agentId: AgentId, skills: AgentSkill[]): Promise<void> =>
    invoke('agent_skills_set', { agentId, skills }),
  /** O que o agente levaria se subisse agora: ativas em ordem e ignoradas com o porquê. */
  plan: (agentId: AgentId): Promise<SkillPlan> => invoke('agent_skills_plan', { agentId }),

  // Editor e biblioteca (T7). `editing` é a pasta da skill aberta; `null` para uma nova.
  /** Valida o rascunho sem escrever nada. */
  check: (source: string, editing: string | null): Promise<SkillCheck> =>
    invoke('skill_check', { source, editing }),
  open: (name: string): Promise<OpenedSkill> => invoke('skill_open', { name }),
  /** Um `SKILL.md` que não carregou, pelo caminho, para ser consertado. */
  openFile: (path: string): Promise<OpenedSkill> => invoke('skill_open_file', { path }),
  save: (source: string, editing: string | null): Promise<Skill> =>
    invoke('skill_save', { source, editing }),
  /** O texto de uma cópia com nome livre; não salva. */
  duplicate: (name: string): Promise<string> => invoke('skill_duplicate', { name }),
  remove: (dir: string): Promise<void> => invoke('skill_delete', { dir }),
  /** Uma pasta de skill (ou o `SKILL.md` dela) copiada para a biblioteca. */
  importFrom: (path: string): Promise<Skill> => invoke('skill_import', { path }),
  /** Escreve em `<to>/<nome>/` e devolve a pasta criada. */
  exportTo: (name: string, to: string): Promise<string> => invoke('skill_export', { name, to }),
  /** Quem tem a skill, com equipe e estado — a lista do "precisam reiniciar". */
  users: (name: string): Promise<SkillUser[]> => invoke('skill_users', { name }),
};

/** A biblioteca mudou no disco (hot-reload): peça a lista de novo. */
export function onSkillsChanged(handler: () => void): Promise<UnlistenFn> {
  return listen('skills:changed', () => handler());
}
