import { useCallback, useEffect, useState } from 'react';
import { onAgentState } from '@/features/agents/api';
import type { AgentState } from '@/types/generated/AgentState';
import type { AgentSummary } from '@/types/generated/AgentSummary';
import type { StateConfidence } from '@/types/generated/StateConfidence';

interface Live {
  state: AgentState;
  confidence: StateConfidence;
}

/** Uma mudança de estado vista nesta sessão da janela ("últimos eventos" do inspetor). */
export interface StateEvent {
  state: AgentState;
  confidence: StateConfidence;
  at: number;
}

/** Quantas mudanças guardar por agente: é um rastro recente, não um histórico. */
export const EVENTS_KEPT = 20;

/**
 * Estado de cada agente direto do evento `agent:state`, sem esperar a lista de equipes
 * voltar do core (F03-07: a sidebar reflete a mudança em <200 ms). O resumo da equipe
 * só vale até o primeiro evento de cada agente — depois, o evento é sempre mais novo.
 */
export function useLiveStates(summary: AgentSummary[]) {
  const [live, setLive] = useState<Record<string, Live>>({});
  const [events, setEvents] = useState<Record<string, StateEvent[]>>({});

  useEffect(() => {
    const unlisten = onAgentState(({ agentId, state, confidence }) => {
      setLive((current) => ({ ...current, [agentId]: { state, confidence } }));
      const event = { state, confidence, at: Date.now() };
      setEvents((current) => ({
        ...current,
        [agentId]: [event, ...(current[agentId] ?? [])].slice(0, EVENTS_KEPT),
      }));
    });
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
  const eventsOf = useCallback((id: string): StateEvent[] => events[id] ?? [], [events]);
  return { stateOf, confidenceOf, eventsOf };
}
