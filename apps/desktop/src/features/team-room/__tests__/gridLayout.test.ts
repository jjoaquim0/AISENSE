import { describe, expect, it } from 'vitest';
import {
  clampRect,
  FREE_MIN,
  FREE_UNITS,
  freeRectOf,
  hiddenIds,
  presetFor,
  promote,
  readGridLayout,
  reorder,
  setFreeRect,
  visibleIds,
  writeGridLayout,
} from '../gridLayout';

describe('layout da vista Grid', () => {
  it('escolhe o menor preset que mostra todo mundo', () => {
    expect([0, 1, 2, 3, 4, 5, 6, 7, 9, 12].map(presetFor)).toEqual([
      '1',
      '1',
      '2',
      '3',
      '4',
      '6',
      '6',
      '9',
      '9',
      '9',
    ]);
  });

  it('reconcilia o layout salvo com os agentes de agora', () => {
    const saved = { grid: { preset: '4', order: ['c', 'gone', 'a', 'c'] }, flow: { zoom: 2 } };
    const layout = readGridLayout(saved, ['a', 'b', 'c']);
    expect(layout.order).toEqual(['c', 'a', 'b']);
    expect(layout.preset).toBe('4');
  });

  it('layout salvo inválido vira o padrão em vez de quebrar a tela', () => {
    for (const bad of [null, 'x', { grid: { preset: '5' } }, { grid: { order: 'a' } }]) {
      const layout = readGridLayout(bad, ['a', 'b']);
      expect(layout).toEqual({ preset: '2', order: ['a', 'b'], free: {} });
    }
  });

  it('gravar o grid preserva o que outras vistas guardam em teams.layout', () => {
    const grid = readGridLayout({}, ['a']);
    expect(writeGridLayout({ flow: { zoom: 2 } }, grid)).toEqual({ flow: { zoom: 2 }, grid });
  });

  it('nos presets só os N primeiros aparecem; o resto fica oculto', () => {
    const layout = { preset: '2' as const, order: ['a', 'b', 'c', 'd'], free: {} };
    expect(visibleIds(layout)).toEqual(['a', 'b']);
    expect(hiddenIds(layout)).toEqual(['c', 'd']);
    expect(visibleIds({ ...layout, preset: 'free' })).toEqual(['a', 'b', 'c', 'd']);
  });

  it('arrastar leva o painel para o lugar do alvo', () => {
    expect(reorder(['a', 'b', 'c', 'd'], 'a', 'c')).toEqual(['b', 'c', 'a', 'd']);
    expect(reorder(['a', 'b', 'c', 'd'], 'd', 'a')).toEqual(['d', 'a', 'b', 'c']);
    expect(reorder(['a', 'b'], 'a', 'zz')).toEqual(['a', 'b']);
  });

  it('mostrar um oculto o coloca no último lugar visível', () => {
    const layout = { preset: '2' as const, order: ['a', 'b', 'c', 'd'], free: {} };
    expect(promote(layout, 'd').order).toEqual(['a', 'd', 'b', 'c']);
    expect(promote(layout, 'a')).toBe(layout);
  });

  it('modo livre: mosaico padrão, encaixe e limites', () => {
    const layout = readGridLayout({}, ['a', 'b', 'c', 'd']);
    expect(freeRectOf(layout, 'd')).toEqual({ x: 0, y: 8, w: 8, h: 8 });
    expect(clampRect({ x: -3, y: 30, w: 1.4, h: 99 })).toEqual({
      x: 0,
      y: 0,
      w: FREE_MIN,
      h: FREE_UNITS,
    });
    const moved = setFreeRect(layout, 'a', { x: 20.6, y: 2.2, w: 8, h: 8 });
    expect(moved.free.a).toEqual({ x: 16, y: 2, w: 8, h: 8, z: 1 });
  });

  it('o último painel mexido no modo livre fica por cima', () => {
    let layout = readGridLayout({}, ['a', 'b']);
    layout = setFreeRect(layout, 'a', { x: 0, y: 0, w: 8, h: 8 });
    layout = setFreeRect(layout, 'b', { x: 0, y: 0, w: 8, h: 8 });
    expect(layout.free.b?.z).toBeGreaterThan(layout.free.a?.z ?? 0);
    layout = setFreeRect(layout, 'a', { x: 1, y: 1, w: 8, h: 8 });
    expect(layout.free.a?.z).toBeGreaterThan(layout.free.b?.z ?? 0);
  });
});
