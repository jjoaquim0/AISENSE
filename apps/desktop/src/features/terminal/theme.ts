import type { ITheme } from '@xterm/xterm';
import { isDark, type Theme } from '@/lib/theme';
import { parseOklch, toHex } from '@/styles/color';

/**
 * Tema do xterm derivado dos tokens de `tokens.css`.
 *
 * Uma única fonte de cor para o app inteiro: mudar o token muda o terminal junto, e o
 * teste de contraste cobre as 16 cores ANSI (docs/08-design-system.md).
 *
 * Os fallbacks existem para o caso de um token sumir — e são **por tema**, porque um
 * fallback escuro num terminal claro seria pior que a cor faltando.
 */
const FALLBACK = {
  dark: {
    background: '#141816',
    foreground: '#eef0ed',
    accent: '#2ee68a',
    selection: '#3a3a45',
    black: '#45454f',
    red: '#ff6b6b',
    green: '#5ed49a',
    yellow: '#e8c16a',
    blue: '#7aa2f7',
    magenta: '#d18bf0',
    cyan: '#6fd0dd',
    white: '#e9e9ee',
    brightBlack: '#6b6b78',
    brightRed: '#ff8f8f',
    brightGreen: '#7ae6b4',
    brightYellow: '#f5d98a',
    brightBlue: '#9bbcff',
    brightMagenta: '#e0a8ff',
    brightCyan: '#95e5ef',
    brightWhite: '#fbfbfd',
  },
  light: {
    background: '#fcfdfc',
    foreground: '#0c0f0d',
    accent: '#087a42',
    selection: '#d9d9e0',
    black: '#2a2a33',
    red: '#c0342e',
    green: '#2f6d44',
    yellow: '#8a6216',
    blue: '#2f57c9',
    magenta: '#a32c9e',
    cyan: '#1f6a75',
    white: '#d0d0d8',
    brightBlack: '#5a5a66',
    brightRed: '#a72722',
    brightGreen: '#25603a',
    brightYellow: '#75530f',
    brightBlue: '#2545ad',
    brightMagenta: '#8c2287',
    brightCyan: '#175a64',
    brightWhite: '#3f3f4a',
  },
} as const;

export function readTerminalTheme(theme: Theme): ITheme {
  const styles = getComputedStyle(document.documentElement);
  const fallback = isDark(theme) ? FALLBACK.dark : FALLBACK.light;

  const read = (name: string, backup: string): string => {
    const color = parseOklch(styles.getPropertyValue(name).trim());
    return color ? toHex(color) : backup;
  };

  const background = read('--bg-terminal', fallback.background);

  return {
    background,
    foreground: read(isDark(theme) ? '--ansi-white' : '--ansi-black', fallback.foreground),
    cursor: read('--emphasis', fallback.accent),
    cursorAccent: background,
    selectionBackground: read('--bg-active', fallback.selection),

    black: read('--ansi-black', fallback.black),
    red: read('--ansi-red', fallback.red),
    green: read('--ansi-green', fallback.green),
    yellow: read('--ansi-yellow', fallback.yellow),
    blue: read('--ansi-blue', fallback.blue),
    magenta: read('--ansi-magenta', fallback.magenta),
    cyan: read('--ansi-cyan', fallback.cyan),
    white: read('--ansi-white', fallback.white),

    brightBlack: read('--ansi-bright-black', fallback.brightBlack),
    brightRed: read('--ansi-bright-red', fallback.brightRed),
    brightGreen: read('--ansi-bright-green', fallback.brightGreen),
    brightYellow: read('--ansi-bright-yellow', fallback.brightYellow),
    brightBlue: read('--ansi-bright-blue', fallback.brightBlue),
    brightMagenta: read('--ansi-bright-magenta', fallback.brightMagenta),
    brightCyan: read('--ansi-bright-cyan', fallback.brightCyan),
    brightWhite: read('--ansi-bright-white', fallback.brightWhite),
  };
}
