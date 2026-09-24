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

/**
 * Atalhos remapeáveis (T9 → Atalhos; F08-05). A chave é o atalho padrão, que é como
 * cada tela registra seus `Bindings`; o remapeamento traduz a tecla na entrada.
 */
export const SHORTCUT_CATALOG: { combo: string; label: string; group: string }[] = [
  { combo: '⌘K', label: 'Paleta de comandos', group: 'Geral' },
  { combo: '⌘B', label: 'Mostrar/ocultar lista de agentes', group: 'Geral' },
  { combo: '⌘I', label: 'Mostrar/ocultar inspetor', group: 'Geral' },
  { combo: '⌘⇧D', label: 'Alternar tema claro/escuro', group: 'Geral' },
  { combo: '⌘,', label: 'Configurações', group: 'Geral' },
  { combo: '⌘G', label: 'Trocar a vista da Sala da Equipe', group: 'Sala da Equipe' },
  { combo: '⌘T', label: 'Novo agente', group: 'Sala da Equipe' },
  { combo: '⌘W', label: 'Fechar o painel (não para o agente)', group: 'Sala da Equipe' },
  { combo: '⌘\\', label: 'Dividir / voltar para a grade', group: 'Sala da Equipe' },
];

/**
 * Tradução de um remapeamento `{padrão: escolhido}` para a direção da tecla:
 * `escolhido → padrão`, e o padrão remapeado deixa de valer (senão a mesma ação teria
 * duas teclas e a antiga ficaria "ocupada" à toa).
 */
export interface Remap {
  toDefault: Map<string, string>;
  disabled: Set<string>;
}

export function buildRemap(overrides: Record<string, string>): Remap {
  const toDefault = new Map<string, string>();
  const disabled = new Set<string>();
  for (const [from, to] of Object.entries(overrides)) {
    if (!to || from === to) continue;
    toDefault.set(to, from);
    disabled.add(from);
  }
  return { toDefault, disabled };
}

/** O atalho padrão que a tecla `combo` aciona, ou `null` se ela foi remapeada para longe. */
export function resolveCombo(combo: string, remap: Remap): string | null {
  const mapped = remap.toDefault.get(combo);
  if (mapped) return mapped;
  return remap.disabled.has(combo) ? null : combo;
}

/** Texto `⌘⇧X` de um evento, para gravar um atalho novo. `null` para teclas soltas. */
export function recordCombo(event: KeyLike): string | null {
  if (['Meta', 'Control', 'Shift', 'Alt'].includes(event.key)) return null;
  const combo = comboOf(event);
  return combo?.startsWith('⌘') ? combo : null;
}
