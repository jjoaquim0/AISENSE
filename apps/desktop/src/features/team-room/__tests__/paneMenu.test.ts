import { describe, expect, it } from 'vitest';
import type { AgentState } from '@/types/generated/AgentState';
import { paneMenu } from '../paneMenu';

const actions = (state: AgentState) => paneMenu(state).map((i) => i.action);
const enabled = (state: AgentState) =>
  paneMenu(state)
    .filter((i) => !i.disabled)
    .map((i) => i.action);

describe('menu ⋮ do painel', () => {
  it('cobre reiniciar, parar, limpar, duplicar e configurar com o agente rodando', () => {
    expect(actions('busy')).toEqual(['restart', 'stop', 'clear', 'duplicate', 'configure']);
    expect(enabled('busy')).toEqual(['restart', 'stop', 'clear', 'duplicate', 'configure']);
  });

  it('troca parar por iniciar quando o agente está parado', () => {
    expect(actions('stopped')).toEqual(['restart', 'start', 'clear', 'duplicate', 'configure']);
    expect(enabled('stopped')).toEqual(['start', 'duplicate', 'configure']);
  });

  it('deixa limpar o histórico de um agente que caiu', () => {
    expect(enabled('failed')).toContain('clear');
    expect(enabled('failed')).not.toContain('restart');
  });

  it('mantém a posição dos itens em todos os estados', () => {
    const states: AgentState[] = [
      'starting',
      'idle',
      'busy',
      'awaiting_input',
      'failed',
      'stopped',
    ];
    for (const state of states) {
      expect(paneMenu(state)).toHaveLength(5);
      expect(actions(state)[2]).toBe('clear');
    }
  });
});
