import { describe, expect, it } from 'vitest';
import type { TeamProgress } from '@/types/generated/TeamProgress';
import { confirmationFor, describeProgress, progressRatio } from '../teamProgress';

const p = (over: Partial<TeamProgress>): TeamProgress => ({
  teamId: 'tem_1',
  op: 'start',
  done: 0,
  total: 6,
  agentId: null,
  finished: false,
  ...over,
});

describe('progresso dos controles da equipe', () => {
  it('descreve quem está sendo tratado e em que posição', () => {
    const handle = (id: string) => (id === 'agt_3' ? 'backend' : id);
    expect(describeProgress(p({ done: 2, agentId: 'agt_3' }), handle)).toBe(
      'Iniciando 3/6 · @backend',
    );
    expect(describeProgress(p({ op: 'stop', done: 0, total: 2 }), handle)).toBe('Parando 1/2');
    expect(describeProgress(p({ op: 'restart', done: 6, finished: true }), handle)).toBe(
      'Reiniciando 6/6',
    );
  });

  it('calcula a fração sem dividir por zero', () => {
    expect(progressRatio(p({ done: 3 }))).toBe(0.5);
    expect(progressRatio(p({ total: 0 }))).toBe(1);
  });

  it('só confirma quando há agente rodando', () => {
    expect(confirmationFor('stop', 0)).toBeNull();
    expect(confirmationFor('restart', 0)).toBeNull();
    expect(confirmationFor('stop', 1)).toContain('Parar 1 agente');
    expect(confirmationFor('restart', 4)).toContain('Reiniciar 4 agentes');
  });
});
