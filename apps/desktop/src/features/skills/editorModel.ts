import type { SkillEntry } from '@/types/generated/SkillEntry';
import type { SkillProblem } from '@/types/generated/SkillProblem';
import type { SkillUser } from '@/types/generated/SkillUser';

/** Regras da Biblioteca de Skills (T7) e do editor, sem React — testadas à parte. */

/** Ponto de partida de "+ Nova skill": válido, com os campos do docs/06 comentados. */
export const NEW_SKILL_TEMPLATE = `---
name: minha-skill
description: Uma frase dizendo quando usar esta skill.
version: 1.0.0
# targets: [claude, codex]   # vazio = todos os runtimes
# priority: 50               # ordem de injeção; menor vem primeiro
---

# Minha skill

Descreva o papel e como o agente deve trabalhar.

## Como trabalhar
1. Primeiro passo.
2. Segundo passo.
`;

export { ALWAYS_ON_SKILL } from './assign';

export interface LibrarySections {
  builtin: SkillEntry[];
  user: SkillEntry[];
  /** Atribuídas a alguém, mas o `SKILL.md` sumiu do disco. */
  missing: SkillEntry[];
}

/** Separa por origem e filtra pela busca (nome ou descrição, sem acento nem caixa). */
export function librarySections(skills: SkillEntry[], query: string): LibrarySections {
  const needle = fold(query.trim());
  const match = (s: SkillEntry) =>
    needle === '' || fold(s.name).includes(needle) || fold(s.description).includes(needle);
  const sections: LibrarySections = { builtin: [], user: [], missing: [] };
  for (const skill of skills.filter(match)) {
    if (skill.source === null) sections.missing.push(skill);
    else if (skill.source.kind === 'builtin') sections.builtin.push(skill);
    else sections.user.push(skill);
  }
  return sections;
}

function fold(text: string): string {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();
}

/** "1 agente", "3 agentes", "nenhum agente". */
export function agentCount(n: number): string {
  if (n === 0) return 'nenhum agente';
  return n === 1 ? '1 agente' : `${n} agentes`;
}

/** Ligada **e** rodando: só estes precisam reiniciar para ver a mudança. */
export function needsRestart(users: SkillUser[]): SkillUser[] {
  return users.filter((u) => u.enabled && u.running);
}

/** "@bia (Squad Produto)". */
export function describeUser(user: SkillUser): string {
  return user.teamName ? `@${user.handle} (${user.teamName})` : `@${user.handle}`;
}

/** A frase da barra inferior do editor: quantos usam e quem precisa reiniciar. */
export function impactSummary(users: SkillUser[]): string {
  if (users.length === 0) return 'Nenhum agente usa esta skill.';
  const restart = needsRestart(users);
  const used = `Usada por ${agentCount(users.length)}.`;
  if (restart.length === 0) return `${used} Nenhum está rodando com ela: vale no próximo início.`;
  const who = restart.map(describeUser).join(', ');
  return restart.length === 1
    ? `${used} 1 agente precisa reiniciar para aplicar: ${who}.`
    : `${used} ${restart.length} agentes precisam reiniciar para aplicar: ${who}.`;
}

export type BudgetLevel = 'ok' | 'near' | 'over';

/** Cor do contador: amarelo a partir de 80% do limite do `BOOT.md`, vermelho acima. */
export function budgetLevel(chars: number, limit: number): BudgetLevel {
  if (chars > limit) return 'over';
  return chars * 100 >= limit * 80 ? 'near' : 'ok';
}

const numbers = new Intl.NumberFormat('pt-BR');

export function formatCount(n: number): string {
  return numbers.format(n);
}

/** `caminho:linha: mensagem` — o mesmo formato dos problemas da biblioteca. */
export function describeProblem(problem: SkillProblem): string {
  const line = problem.line === null ? '' : `:${problem.line}`;
  return `${problem.path}${line}: ${problem.message}`;
}

/** O corpo Markdown, sem o frontmatter — para o preview enquanto o YAML ainda não valida. */
export function bodyOf(source: string): string {
  const text = source.replace(/^﻿/, '');
  const match = /^---\r?\n[\s\S]*?\r?\n---[ \t]*(?:\r?\n|$)/.exec(text);
  return match ? text.slice(match[0].length) : text;
}
