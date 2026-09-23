/**
 * Camada de atalhos (docs/08, "Atalhos de teclado"; F03-08). Regras puras aqui, o
 * listener em `useShortcuts.ts`.
 *
 * O problema que esta camada resolve: com um terminal focado, a tecla vai para o
 * textarea escondido do xterm, que a consome e a manda para o shell. O listener roda na
 * fase de **captura** da janela — antes do xterm — e, quando o atalho é do app, a tecla
 * para ali e não chega ao processo.
 *
 * Mas no Linux e no Windows `⌘` é `Ctrl`, e `Ctrl+W`, `Ctrl+B`, `Ctrl+\`... são teclas
 * do shell (apagar palavra, prefixo do tmux, SIGQUIT). Então, dentro do terminal, fora do
 * macOS só valem os atalhos marcados `inTerminal: 'always'` (⌘1..9, sem uso no shell);
 * os outros ficam para depois de `Esc Esc` (ADR 0007).
 */

export interface Shortcut {
  run: (event: KeyboardEvent) => void;
  /**
   * Dentro de um terminal focado: `'always'` vale em todo SO; `'mac'` (padrão) só no
   * macOS, onde `⌘` nunca é tecla de shell.
   */
  inTerminal?: 'always' | 'mac';
}

export type Bindings = Record<string, Shortcut | Shortcut['run']>;

/** Duas `Esc` dentro deste intervalo, no terminal, devolvem o foco à interface. */
export const DOUBLE_ESCAPE_MS = 400;

export const isMacPlatform = (): boolean =>
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);

type KeyLike = Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey'>;

/**
 * `⌘1`, `⌘⇧D`, `⌘\`... Dígitos vêm do `code` físico: no AZERTY o `1` sem Shift é `&`,
 * e `⌘1` tem de ser a mesma tecla em qualquer layout.
 */
export function comboOf(event: KeyLike): string | null {
  if (event.altKey) return null;
  const mod = event.metaKey || event.ctrlKey;
  const digit = /^Digit(\d)$/.exec(event.code)?.[1];
  const key = digit ?? (event.key.length === 1 ? event.key.toUpperCase() : event.key);
  return `${mod ? '⌘' : ''}${event.shiftKey ? '⇧' : ''}${key}`;
}

export function normalize(binding: Shortcut | Shortcut['run']): Shortcut {
  return typeof binding === 'function' ? { run: binding } : binding;
}

/** O atalho vale aqui? Dentro do terminal, só se não roubar uma tecla do shell. */
export function applies(shortcut: Shortcut, inTerminal: boolean, mac: boolean): boolean {
  if (!inTerminal) return true;
  return mac || shortcut.inTerminal === 'always';
}
