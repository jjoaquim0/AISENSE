import { useCallback, useEffect, useState } from 'react';
import { onAgentState } from '@/features/agents/api';
import type { AgentState } from '@/types/generated/AgentState';
import type { AgentSummary } from '@/types/generated/AgentSummary';
import type { StateConfidence } from '@/types/generated/StateConfidence';

interface Live {
  state: AgentState;
  confidence: StateConfidence;
}

/**
 * Estado de cada agente direto do evento `agent:state`, sem esperar a lista de equipes
 * voltar do core (F03-07: a sidebar reflete a mudança em <200 ms). O resumo da equipe
 * só vale até o primeiro evento de cada agente — depois, o evento é sempre mais novo.
 */
export function useLiveStates(summary: AgentSummary[]) {
  const [live, setLive] = useState<Record<string, Live>>({});

  useEffect(() => {
    const unlisten = onAgentState(({ agentId, state, confidence }) =>
      setLive((current) => ({ ...current, [agentId]: { state, confidence } })),
    );
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  const stateOf = useCallback(
    (id: string): AgentState =>
      live[id]?.state ?? summary.find((a) => a.id === id)?.state ?? 'stopped',
    [live, summary],
  );
  const confidenceOf = useCallback(
    (id: string): StateConfidence | undefined => live[id]?.confidence,
    [live],
  );
  return { stateOf, confidenceOf };
}
