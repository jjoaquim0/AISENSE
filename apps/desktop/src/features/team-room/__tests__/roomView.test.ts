import { describe, expect, it } from 'vitest';
import { focusTarget, nextRoomView, readRoomView } from '../roomView';

describe('vista da Sala da Equipe', () => {
  it('lê a vista salva e cai para Grid no que não conhece', () => {
    expect(readRoomView({ view: 'focus' })).toBe('focus');
    expect(readRoomView({ view: 'flow' })).toBe('grid');
    expect(readRoomView(null)).toBe('grid');
  });

  it('cicla entre as vistas', () => {
    expect(nextRoomView('grid')).toBe('focus');
    expect(nextRoomView('focus')).toBe('grid');
  });

  it('o painel grande mostra o agente em foco, ou o primeiro', () => {
    expect(focusTarget(['a', 'b'], 'b')).toBe('b');
    expect(focusTarget(['a', 'b'], 'sumiu')).toBe('a');
    expect(focusTarget([], null)).toBeNull();
  });
});
