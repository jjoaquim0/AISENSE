import { describe, expect, it } from 'vitest';
import type { PlannedAgent } from '@/types/generated/PlannedAgent';
import type { RuntimeInfo } from '@/types/generated/RuntimeInfo';
import { withRunnableRuntimes } from '../plan';

const planned = (handle: string, adapterId: string): PlannedAgent =>
  ({
    draft: { handle, adapterId } as PlannedAgent['draft'],
    preferredAdapterId: adapterId,
    unavailable: false,
  }) as PlannedAgent;

const runtime = (id: string, installed: boolean): RuntimeInfo =>
  ({
    adapter: { id } as RuntimeInfo['adapter'],
    status: installed
      ? { status: 'available', path: `/bin/${id}`, version: null }
      : { status: 'missing', reason: 'não encontrado' },
  }) as RuntimeInfo;

describe('withRunnableRuntimes', () => {
  it('troca pelo shell só quem não tem o runtime instalado', () => {
    const out = withRunnableRuntimes(
      [planned('dev', 'claude'), planned('revisor', 'codex')],
      [runtime('claude', true), runtime('codex', false), runtime('shell', true)],
    );
    expect(out.map((p) => p.draft.adapterId)).toEqual(['claude', 'shell']);
    expect(out[1]?.preferredAdapterId).toBe('codex');
  });

  it('sem shell disponível, mantém o plano como veio', () => {
    const plan = [planned('dev', 'claude')];
    expect(withRunnableRuntimes(plan, [runtime('claude', false)])).toEqual(plan);
  });
});
