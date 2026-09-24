/** Regras das notas no front, sem React (docs/15). */

/** "Decisões técnicas" → "decisoes-tecnicas": o slug que o core aceita. */
export function slugify(title: string): string {
  return title
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64)
    .replace(/-+$/g, '');
}

/** O mesmo formato do core: `^[a-z0-9][a-z0-9-]*$`, até 64. */
export function isValidSlug(slug: string): boolean {
  return /^[a-z0-9][a-z0-9-]*$/.test(slug) && slug.length <= 64;
}

const relative = new Intl.RelativeTimeFormat('pt-BR', { numeric: 'auto' });

/** "há 5 minutos", "ontem". */
export function updatedLabel(epochMs: number, now = Date.now()): string {
  const seconds = Math.round((epochMs - now) / 1000);
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ['day', 86_400],
    ['hour', 3_600],
    ['minute', 60],
  ];
  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size) return relative.format(Math.round(seconds / size), unit);
  }
  return 'agora';
}
