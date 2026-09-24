import { describe, expect, it } from 'vitest';
import type { AppSettings } from '@/types/generated/AppSettings';
import { stateOfField } from '../calibration';
import { terminalFontOf } from '../store';

const base = {
  appearance: {
    theme: 'system',
    density: 'comfortable',
    terminalFontSize: 15,
    terminalFontFamily: '',
  },
} as unknown as AppSettings;

describe('terminalFontOf', () => {
  it('usa os padrões sem preferências carregadas', () => {
    expect(terminalFontOf(null)).toEqual({
      fontFamily: 'var(--font-mono)',
      fontSize: 13,
      lineHeight: 1.4,
    });
  });

  it('fonte escolhida vem antes da do AISENSE, que fica de reserva', () => {
    const font = terminalFontOf({
      ...base,
      appearance: { ...base.appearance, terminalFontFamily: 'Fira "Code"', density: 'compact' },
    });
    expect(font).toEqual({
      fontFamily: '"Fira Code", var(--font-mono)',
      fontSize: 15,
      lineHeight: 1.2,
    });
  });
});

describe('stateOfField', () => {
  it('liga cada regex ao estado dele', () => {
    expect(stateOfField('idle_regex')).toBe('idle');
    expect(stateOfField('awaiting_regex')).toBe('awaiting_input');
    expect(stateOfField('outro')).toBeNull();
  });
});
