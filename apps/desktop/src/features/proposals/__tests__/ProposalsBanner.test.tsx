import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/types/generated/Agent';
import type { AgentProposal as Proposal } from '@/types/generated/AgentProposal';

const list = vi.fn();
const decide = vi.fn();
vi.mock('@/features/proposals/api', () => ({
  proposalsApi: {
    list: (...a: unknown[]) => list(...a),
    decide: (...a: unknown[]) => decide(...a),
  },
  onProposalChanged: () => Promise.resolve(() => {}),
}));

const { ProposalsBanner, describeProposal } = await import('../ProposalsBanner');

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const proposal: Proposal = {
  id: 'prp_1',
  teamId: 't1',
  proposedBy: 'a1',
  action: { kind: 'createAgent', handle: '@qa', name: 'QA', role: 'testa', adapterId: 'claude' },
  reason: 'ninguém testa o login',
  state: 'pending',
  createdAt: 0,
  decidedAt: null,
  decisionNote: null,
};

describe('<ProposalsBanner />', () => {
  it('mostra a proposta e só o humano aceita', async () => {
    list.mockResolvedValueOnce([proposal]).mockResolvedValue([]);
    decide.mockResolvedValue({ ...proposal, state: 'accepted' });
    const container = document.createElement('div');
    const root = createRoot(container);
    const agents = [{ id: 'a1', handle: 'coordenador' } as Agent];
    await act(async () => root.render(<ProposalsBanner teamId="t1" agents={agents} />));
    expect(container.textContent).toContain(
      '@coordenador propõe criar o agente @qa (claude) — testa',
    );
    const accept = [...container.querySelectorAll('button')].find(
      (b) => b.textContent === 'Aceitar',
    );
    await act(async () => accept?.click());
    expect(decide).toHaveBeenCalledWith('t1', 'prp_1', true);
    expect(container.textContent).toBe('');
    expect(
      describeProposal({
        ...proposal,
        action: { kind: 'setAutonomy', handle: 'backend', autonomy: 'trusted' },
      }),
    ).toBe('mudar a autonomia de @backend para confiável');
    act(() => root.unmount());
  });
});
