import { Lightbulb } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import type { Agent } from '@/types/generated/Agent';
import type { AgentProposal as Proposal } from '@/types/generated/AgentProposal';
import type { TeamId } from '@/types/generated/TeamId';
import { onProposalChanged, proposalsApi } from './api';

/** Uma linha para a proposta, com o que o agente quer fazer. */
export function describeProposal(p: Proposal): string {
  const a = p.action;
  const at = (h: string) => `@${h.replace(/^@/, '')}`;
  switch (a.kind) {
    case 'createAgent':
      return `criar o agente ${at(a.handle)} (${a.adapterId})${a.role ? ` — ${a.role}` : ''}`;
    case 'setAutonomy':
      return `mudar a autonomia de ${at(a.handle)} para ${a.autonomy === 'trusted' ? 'confiável' : 'perguntar'}`;
    case 'editSkill':
      return `editar a skill ${a.skill}: ${a.change}`;
    case 'changeColumns':
      return `mudar as colunas do quadro: ${a.change}`;
  }
}

/**
 * Propostas pendentes da equipe (F07-02, docs/11): nenhum agente cria agente, muda autonomia,
 * edita skill ou mexe nas colunas sozinho — a proposta espera você aqui.
 */
export function ProposalsBanner({
  teamId,
  agents,
  onDecided,
}: {
  teamId: TeamId;
  agents: Agent[];
  onDecided?: (p: Proposal) => void;
}) {
  const [pending, setPending] = useState<Proposal[]>([]);
  const [problem, setProblem] = useState<string | null>(null);

  const reload = useCallback(
    () =>
      proposalsApi
        .list(teamId)
        .then(setPending)
        .catch(() => {}),
    [teamId],
  );

  useEffect(() => {
    void reload();
    const off = onProposalChanged((p) => {
      if (p.teamId === teamId) void reload();
    });
    return () => {
      void off.then((stop) => stop());
    };
  }, [teamId, reload]);

  if (pending.length === 0) return null;

  const decide = async (p: Proposal, accept: boolean) => {
    setProblem(null);
    try {
      const decided = await proposalsApi.decide(teamId, p.id, accept);
      onDecided?.(decided);
      await reload();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  const handleOf = (id: string | null) => {
    const agent = agents.find((a) => a.id === id);
    return agent ? `@${agent.handle}` : 'um agente removido';
  };

  return (
    <section
      aria-label="Propostas dos agentes"
      className="flex flex-col gap-1 border-b border-subtle px-4 py-2 text-caption"
    >
      {pending.map((p) => (
        <div key={p.id} className="flex items-center gap-2">
          <Lightbulb size={14} className="shrink-0 text-awaiting" />
          <span className="min-w-0 flex-1 text-primary">
            {handleOf(p.proposedBy)} propõe {describeProposal(p)}
            <span className="text-muted"> — {p.reason}</span>
          </span>
          <Button size="sm" variant="ghost" onClick={() => void decide(p, false)}>
            Recusar
          </Button>
          <Button size="sm" variant="primary" onClick={() => void decide(p, true)}>
            Aceitar
          </Button>
        </div>
      ))}
      {problem && (
        <p role="alert" className="text-failed">
          {problem}
        </p>
      )}
    </section>
  );
}
