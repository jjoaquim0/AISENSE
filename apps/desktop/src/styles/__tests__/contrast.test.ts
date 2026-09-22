import { describe, expect, it } from 'vitest';
import { contrastRatio } from '@/styles/color';
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
