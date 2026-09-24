import { describe, expect, it } from 'vitest';
import { contrastRatio, parseOklch, toHex } from '@/styles/color';
import { loadTheme, type ThemeName } from './tokens';

/**
 * Guarda de acessibilidade (docs/08-design-system.md).
 *
 * Este teste existe para falhar quando alguém "só clarear um cinzinha" e deixar
 * texto ilegível. Não desligue: ajuste o token.
 *
 * Mínimos WCAG AA: 4.5:1 para texto normal, 3:1 para texto ≥18px e para elementos
 * de interface não textuais (indicadores, bordas de foco).
 */

const TEXT_MIN = 4.5;
const UI_MIN = 3;

/** [frente, fundo, mínimo, descrição] */
const PAIRS: Array<[string, string, number, string]> = [
  ['--fg-primary', '--bg-base', TEXT_MIN, 'texto principal sobre o fundo da janela'],
  ['--fg-primary', '--bg-surface', TEXT_MIN, 'texto principal sobre card/sidebar'],
  ['--fg-primary', '--bg-raised', TEXT_MIN, 'texto principal sobre popover/modal'],
  ['--fg-primary', '--bg-hover', TEXT_MIN, 'texto principal sobre item em hover'],
  ['--fg-primary', '--bg-active', TEXT_MIN, 'texto principal sobre item selecionado'],
  ['--fg-secondary', '--bg-base', TEXT_MIN, 'texto de apoio sobre o fundo'],
  ['--fg-secondary', '--bg-surface', TEXT_MIN, 'texto de apoio sobre card'],
  ['--fg-muted', '--bg-base', TEXT_MIN, 'metadados sobre o fundo'],
  ['--fg-muted', '--bg-surface', TEXT_MIN, 'metadados sobre card'],
  ['--accent-fg', '--accent', TEXT_MIN, 'texto do botão primário'],
  ['--emphasis', '--bg-base', TEXT_MIN, 'link e texto em verde sobre o fundo'],
  ['--emphasis', '--bg-surface', TEXT_MIN, 'link e texto em verde sobre card'],
  ['--ring', '--bg-base', UI_MIN, 'anel de foco sobre o fundo'],
  ['--ring', '--bg-surface', UI_MIN, 'anel de foco sobre card'],
  ['--border-strong', '--bg-surface', UI_MIN, 'contorno de input'],
  ['--state-idle', '--bg-surface', UI_MIN, 'indicador ocioso'],
  ['--state-busy', '--bg-surface', UI_MIN, 'indicador trabalhando'],
  ['--state-awaiting', '--bg-surface', UI_MIN, 'indicador aguardando'],
  ['--state-failed', '--bg-surface', UI_MIN, 'indicador de erro'],
  ['--state-stopped', '--bg-surface', UI_MIN, 'indicador parado'],
];

const AGENT_COLORS = [
  '--agent-violet',
  '--agent-cyan',
  '--agent-emerald',
  '--agent-amber',
  '--agent-rose',
  '--agent-indigo',
  '--agent-teal',
  '--agent-fuchsia',
];

const THEMES: ThemeName[] = ['light', 'dark'];

describe.each(THEMES)('contraste no tema %s', (theme) => {
  const tokens = loadTheme(theme);

  const ratioOf = (fg: string, bg: string): number => {
    const foreground = tokens[fg];
    const background = tokens[bg];
    if (!foreground) throw new Error(`Token ausente no tema ${theme}: ${fg}`);
    if (!background) throw new Error(`Token ausente no tema ${theme}: ${bg}`);
    return contrastRatio(foreground, background);
  };

  it.each(PAIRS)('%s sobre %s atinge %d:1 (%s)', (fg, bg, min) => {
    const ratio = ratioOf(fg, bg);
    expect(
      ratio,
      `${fg} sobre ${bg} no tema ${theme}: ${ratio.toFixed(2)}:1, mínimo ${min}:1`,
    ).toBeGreaterThanOrEqual(min);
  });

  it.each(AGENT_COLORS)('%s se distingue do fundo do painel', (token) => {
    const ratio = ratioOf(token, '--bg-surface');
    expect(
      ratio,
      `${token} sobre --bg-surface no tema ${theme}: ${ratio.toFixed(2)}:1`,
    ).toBeGreaterThanOrEqual(UI_MIN);
  });

  it.each(AGENT_COLORS)('a inicial no avatar se lê sobre %s', (token) => {
    const ratio = ratioOf('--on-agent', token);
    expect(
      ratio,
      `--on-agent sobre ${token} no tema ${theme}: ${ratio.toFixed(2)}:1`,
    ).toBeGreaterThanOrEqual(TEXT_MIN);
  });

  it('as cores de agente têm peso visual parecido entre si', () => {
    const luminances = AGENT_COLORS.map((token) => ratioOf(token, '--bg-surface'));
    const spread = Math.max(...luminances) / Math.min(...luminances);
    // Se uma cor "pesa" muito mais que outra, o mosaico de terminais fica desequilibrado.
    expect(
      spread,
      `variação de contraste entre as cores de agente: ${spread.toFixed(2)}x`,
    ).toBeLessThan(2);
  });
});

/**
 * Cores ANSI do terminal.
 *
 * É aqui que quase todo tema falha: a paleta ANSI padrão tem azul e preto
 * ilegíveis sobre fundo escuro, e o agente escreve caminho de arquivo e erro
 * justamente nessas cores.
 *
 * `ansi-black` (no escuro) e `ansi-white` (no claro) ficam de fora: em terminal
 * elas são cor de **fundo**, não de texto, e exigir contraste delas contra o
 * próprio fundo não faria sentido.
 */
const ANSI_TEXT_COLORS = [
  '--ansi-red',
  '--ansi-green',
  '--ansi-yellow',
  '--ansi-blue',
  '--ansi-magenta',
  '--ansi-cyan',
  '--ansi-bright-red',
  '--ansi-bright-green',
  '--ansi-bright-yellow',
  '--ansi-bright-blue',
  '--ansi-bright-magenta',
  '--ansi-bright-cyan',
];

describe.each(THEMES)('cores ANSI do terminal no tema %s', (theme) => {
  const tokens = loadTheme(theme);

  const ratioOnTerminal = (token: string): number => {
    const color = tokens[token];
    const background = tokens['--bg-terminal'];
    if (!color) throw new Error(`Token ANSI ausente no tema ${theme}: ${token}`);
    if (!background) throw new Error('--bg-terminal ausente');
    return contrastRatio(color, background);
  };

  it.each(ANSI_TEXT_COLORS)('%s é legível sobre o fundo do terminal', (token) => {
    const ratio = ratioOnTerminal(token);
    expect(
      ratio,
      `${token} sobre --bg-terminal no tema ${theme}: ${ratio.toFixed(2)}:1`,
    ).toBeGreaterThanOrEqual(TEXT_MIN);
  });

  it('a cor de texto padrão do terminal é legível', () => {
    const token = theme === 'dark' ? '--ansi-white' : '--ansi-black';
    expect(ratioOnTerminal(token)).toBeGreaterThanOrEqual(TEXT_MIN);
  });

  it('cinza escuro (usado em texto apagado) ainda é legível', () => {
    // Muitos programas usam ansi-bright-black para texto secundário.
    expect(ratioOnTerminal('--ansi-bright-black')).toBeGreaterThanOrEqual(UI_MIN);
  });

  it('as 16 cores são distinguíveis entre si', () => {
    const seen = new Map<string, string>();
    for (const token of [...ANSI_TEXT_COLORS, '--ansi-black', '--ansi-white']) {
      const color = tokens[token];
      if (!color) throw new Error(`Token ANSI ausente: ${token}`);
      const key = `${color.l.toFixed(3)}|${color.c.toFixed(3)}|${color.h.toFixed(1)}`;
      const previous = seen.get(key);
      expect(previous, `${token} e ${previous} são a mesma cor`).toBeUndefined();
      seen.set(key, token);
    }
  });
});

describe('toHex', () => {
  const hex = (value: string): string => {
    const color = parseOklch(value);
    if (!color) throw new Error(`OKLCH inválido: ${value}`);
    return toHex(color);
  };

  it('converte os extremos', () => {
    expect(hex('oklch(1 0 0)')).toBe('#ffffff');
    expect(hex('oklch(0 0 0)')).toBe('#000000');
  });

  it('produz sempre 7 caracteres', () => {
    for (const token of Object.values(loadTheme('dark'))) {
      expect(toHex(token)).toMatch(/^#[0-9a-f]{6}$/);
    }
  });

  it('recorta cores fora do gamut em vez de gerar valor inválido', () => {
    // Croma alto demais para o gamut sRGB; precisa sair como cor válida mesmo assim.
    expect(hex('oklch(0.7 0.4 150)')).toMatch(/^#[0-9a-f]{6}$/);
  });

  it('mantém a ordem de luminosidade', () => {
    const escuro = Number.parseInt(hex('oklch(0.2 0 0)').slice(1), 16);
    const claro = Number.parseInt(hex('oklch(0.8 0 0)').slice(1), 16);
    expect(claro).toBeGreaterThan(escuro);
  });
});
