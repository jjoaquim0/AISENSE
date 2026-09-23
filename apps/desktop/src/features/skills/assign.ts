import type { AgentSkill } from '@/types/generated/AgentSkill';
import type { SkillEntry } from '@/types/generated/SkillEntry';

/** Regras da lista de skills de um agente — a ordem é a de injeção no `BOOT.md`. */

export function addSkill(list: AgentSkill[], skillId: string): AgentSkill[] {
  if (list.some((s) => s.skillId === skillId)) return list;
  return [...list, { skillId, enabled: true }];
}

export function removeSkill(list: AgentSkill[], skillId: string): AgentSkill[] {
  return list.filter((s) => s.skillId !== skillId);
}

export function toggleSkill(list: AgentSkill[], skillId: string): AgentSkill[] {
  return list.map((s) => (s.skillId === skillId ? { ...s, enabled: !s.enabled } : s));
}

/** Sobe (`-1`) ou desce (`+1`) uma posição; nas pontas, não mexe. */
export function moveSkill(list: AgentSkill[], skillId: string, delta: -1 | 1): AgentSkill[] {
  const from = list.findIndex((s) => s.skillId === skillId);
  const to = from + delta;
  if (from < 0 || to < 0 || to >= list.length) return list;
  const next = [...list];
  const [item] = next.splice(from, 1);
  if (item) next.splice(to, 0, item);
  return next;
}

/** A skill roda neste runtime? `targets` vazio aceita todos (docs/06). */
export function supports(skill: Pick<SkillEntry, 'targets'>, adapterId: string): boolean {
  return skill.targets.length === 0 || skill.targets.includes(adapterId);
}

/** O que ainda dá para atribuir: da biblioteca, carregada do disco e não atribuída. */
export function available(library: SkillEntry[], list: AgentSkill[]): SkillEntry[] {
  return library.filter((s) => s.source !== null && !list.some((a) => a.skillId === s.id));
}
