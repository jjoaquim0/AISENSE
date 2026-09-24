import type { PaletteAction } from './paletteStore';

const RECENT_KEY = 'aisense.palette.recent';
export const RECENT_MAX = 5;

/** Ids usados por último, mais recente primeiro (conveniência deste navegador). */
export function readRecent(): string[] {
  try {
    const raw = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
    return Array.isArray(raw) ? raw.filter((x): x is string => typeof x === 'string') : [];
  } catch {
    return [];
  }
}

export function pushRecent(id: string, current: string[]): string[] {
  const next = [id, ...current.filter((x) => x !== id)].slice(0, RECENT_MAX);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    // Sem armazenamento: a lista só não sobrevive ao reinício.
  }
  return next;
}

/** Recentes que ainda existem nesta tela, na ordem de uso. */
export function recentActions(actions: PaletteAction[], recent: string[]): PaletteAction[] {
  return recent.flatMap((id) => actions.find((a) => a.id === id) ?? []);
}

/**
 * `> enviar @backend texto`, `>@backend texto` ou `> #canal texto`: destino e corpo.
 * Qualquer outra coisa não é envio.
 */
export function parseSend(input: string): { to: string; body: string } | null {
  const m = /^>\s*(?:enviar\s+)?([@#][\w-]+)\s+([\s\S]+)$/.exec(input.trim());
  if (!m?.[1] || !m[2]?.trim()) return null;
  return { to: m[1], body: m[2].trim() };
}

/** Agrupa mantendo a ordem de chegada dos grupos. */
export function grouped(actions: PaletteAction[]): [string, PaletteAction[]][] {
  const out = new Map<string, PaletteAction[]>();
  for (const a of actions) out.set(a.group, [...(out.get(a.group) ?? []), a]);
  return [...out.entries()];
}
