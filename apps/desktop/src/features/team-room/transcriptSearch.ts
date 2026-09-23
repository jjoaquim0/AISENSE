/** Um trecho encontrado na transcrição: `[start, end)` em índices de `string`. */
export type Match = [start: number, end: number];

/** Acima disto a busca para de marcar: realce em 1 MB de texto trava a tela. */
export const MAX_MATCHES = 1000;

/** Todas as ocorrências de `query`, sem diferenciar maiúsculas; vazia não acha nada. */
export function findMatches(text: string, query: string): Match[] {
  const needle = query.trim().toLocaleLowerCase('pt-BR');
  if (!needle) return [];
  const haystack = text.toLocaleLowerCase('pt-BR');
  // `toLocaleLowerCase` pode mudar o comprimento (ex.: "İ"); aí os índices não servem.
  if (haystack.length !== text.length) return findExact(text, query.trim());
  const found: Match[] = [];
  let from = 0;
  while (found.length < MAX_MATCHES) {
    const at = haystack.indexOf(needle, from);
    if (at < 0) break;
    found.push([at, at + needle.length]);
    from = at + needle.length;
  }
  return found;
}

function findExact(text: string, needle: string): Match[] {
  const found: Match[] = [];
  let from = 0;
  while (found.length < MAX_MATCHES) {
    const at = text.indexOf(needle, from);
    if (at < 0) break;
    found.push([at, at + needle.length]);
    from = at + needle.length;
  }
  return found;
}

/** Pedaços para renderizar: texto puro intercalado com as ocorrências. */
export function splitByMatches(
  text: string,
  matches: Match[],
): { text: string; match: number | null }[] {
  const parts: { text: string; match: number | null }[] = [];
  let cursor = 0;
  matches.forEach(([start, end], index) => {
    if (start > cursor) parts.push({ text: text.slice(cursor, start), match: null });
    parts.push({ text: text.slice(start, end), match: index });
    cursor = end;
  });
  if (cursor < text.length) parts.push({ text: text.slice(cursor), match: null });
  return parts;
}

/** Nome sugerido ao exportar: `backend-2026-09-23-1530.txt`. */
export function exportFileName(handle: string, startedAt: number): string {
  const d = new Date(startedAt);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${handle}-${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}.txt`;
}

/** "3 min", "1 h 05 min" — tempo ativo no inspetor. */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h} h ${String(m).padStart(2, '0')} min`;
  if (m > 0) return `${m} min`;
  return `${s} s`;
}
