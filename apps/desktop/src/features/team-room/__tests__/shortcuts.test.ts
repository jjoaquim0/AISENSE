import { describe, expect, it, vi } from 'vitest';
import type { Shortcut } from '@/lib/shortcuts';
import { closePane, type GridLayout, splitLayout, visibleIds } from '../gridLayout';
import { type RoomContext, roomShortcuts } from '../shortcuts';

const layout = (preset: GridLayout['preset'], order = ['a', 'b', 'c', 'd', 'e']): GridLayout => ({
  preset,
  order,
  free: {},
});

describe('closePane (⌘W)', () => {
  it('tira o painel da grade e encolhe o preset, sem perder os outros', () => {
    const next = closePane(layout('4'), 'b');
    expect(next.preset).toBe('3');
    expect(visibleIds(next)).toEqual(['a', 'c', 'd']);
    expect(next.order).toContain('b');
  });
  it('não fecha o último painel, um oculto ou no modo livre', () => {
    const one = layout('1');
    expect(closePane(one, 'a')).toBe(one);
    const two = layout('2');
    expect(closePane(two, 'e')).toBe(two);
    const free = layout('free');
    expect(closePane(free, 'a')).toBe(free);
  });
});

describe('splitLayout (⌘\\)', () => {
  it('vai para o próximo preset maior e para no 9', () => {
    expect(splitLayout(layout('1')).preset).toBe('2');
    expect(splitLayout(layout('4')).preset).toBe('6');
    const nine = layout('9');
    expect(splitLayout(nine)).toBe(nine);
  });
});

function room(partial: Partial<RoomContext> = {}): RoomContext {
  return {
    grid: layout('2'),
    view: 'grid',
    selectedId: 'a',
    setGrid: vi.fn(),
    setView: vi.fn(),
    select: vi.fn(),
    focusPane: vi.fn(),
    newAgent: vi.fn(),
    ...partial,
  };
}
const run = (binding: Shortcut | Shortcut['run'] | undefined) =>
  (typeof binding === 'function' ? binding : binding?.run)?.(new KeyboardEvent('keydown'));

describe('atalhos da sala', () => {
  it('⌘N foca o painel N e traz de volta à grade quem estava fora', () => {
    const ctx = room();
    const bindings = roomShortcuts(ctx);
    run(bindings['⌘4']);
    expect(ctx.select).toHaveBeenCalledWith('d');
    expect(ctx.focusPane).toHaveBeenCalledWith('d');
    expect(visibleIds(vi.mocked(ctx.setGrid).mock.calls[0]?.[0] as GridLayout)).toContain('d');
    run(bindings['⌘9']);
    expect(ctx.select).toHaveBeenCalledTimes(1);
  });

  it('⌘1..9 valem com o terminal focado em todo SO; os outros não', () => {
    const bindings = roomShortcuts(room());
    expect(bindings['⌘1']).toMatchObject({ inTerminal: 'always' });
    expect(typeof bindings['⌘W']).toBe('function');
  });

  it('⌘G alterna a vista, ⌘T abre um agente novo', () => {
    const ctx = room({ view: 'grid' });
    const bindings = roomShortcuts(ctx);
    run(bindings['⌘G']);
    run(bindings['⌘T']);
    expect(ctx.setView).toHaveBeenCalledWith('focus');
    expect(ctx.newAgent).toHaveBeenCalled();
  });

  it('⌘W fecha o painel focado e leva o foco ao primeiro que ficou', () => {
    const ctx = room({ grid: layout('2'), selectedId: 'a' });
    run(roomShortcuts(ctx)['⌘W']);
    expect(vi.mocked(ctx.setGrid).mock.calls[0]?.[0]).toMatchObject({ preset: '1' });
    expect(ctx.focusPane).toHaveBeenCalledWith('b');
  });

  it('⌘\\ divide na grade e, no Foco, volta para a grade', () => {
    const grid = room({ grid: layout('2') });
    run(roomShortcuts(grid)['⌘\\']);
    expect(grid.setGrid).toHaveBeenCalledWith(expect.objectContaining({ preset: '3' }));
    const focus = room({ view: 'focus' });
    run(roomShortcuts(focus)['⌘\\']);
    expect(focus.setView).toHaveBeenCalledWith('grid');
  });
});
