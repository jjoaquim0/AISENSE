/** Junta classes ignorando valores falsos. Mantido mínimo de propósito. */
export function cn(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(' ');
}
