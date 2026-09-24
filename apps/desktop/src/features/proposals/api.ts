import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentProposal as Proposal } from '@/types/generated/AgentProposal';
import type { ProposalId } from '@/types/generated/ProposalId';
import type { TeamId } from '@/types/generated/TeamId';

/** Propostas de ações estruturais (docs/11, F07-02): só você aceita. */
export const proposalsApi = {
  list: (teamId: TeamId, pendingOnly = true): Promise<Proposal[]> =>
    invoke('proposals_list', { teamId, pendingOnly }),
  decide: (teamId: TeamId, id: ProposalId, accept: boolean, note?: string): Promise<Proposal> =>
    invoke('proposal_decide', { teamId, id, accept, note: note ?? null }),
};

export function onProposalChanged(handler: (p: Proposal) => void): Promise<UnlistenFn> {
  return listen<Proposal>('proposal:changed', ({ payload }) => handler(payload));
}
