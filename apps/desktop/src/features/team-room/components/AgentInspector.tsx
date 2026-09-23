import { useCallback, useEffect, useState } from 'react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui';
import { AgentForm } from '@/features/agents/AgentForm';
import { agentsApi } from '@/features/agents/api';
import { errorMessage } from '@/features/teams/api';
import type { Agent } from '@/types/generated/Agent';
import type { AgentState } from '@/types/generated/AgentState';
import type { SessionSummary } from '@/types/generated/SessionSummary';
import type { StateConfidence } from '@/types/generated/StateConfidence';
import type { StateEvent } from '../hooks/useLiveStates';
import { isRunning } from '../sidebar';
import { LogsTab } from './inspector/LogsTab';
import { OverviewTab } from './inspector/OverviewTab';

export type InspectorTab = 'overview' | 'config' | 'logs';

interface AgentInspectorProps {
  agent: Agent | null;
  state: AgentState;
  confidence?: StateConfidence;
  events: StateEvent[];
  /** Diretório da equipe, quando o agente não tem um próprio. */
  teamWorkdir: string;
  teamId: string;
  /** A equipe inteira, para a aba Config validar o endereço. */
  siblings: Agent[];
  onSaved: (agent: Agent, restartRequired: boolean) => void;
}

/**
 * Inspetor do agente (docs/09, T6) no painel direito: abas Visão, Config e Logs. Skills
 * e Caixa entram nas Fases 04 e 05.
 */
export function AgentInspector({
  agent,
  state,
  confidence,
  events,
  teamWorkdir,
  teamId,
  siblings,
  onSaved,
}: AgentInspectorProps) {
  const [tab, setTab] = useState<InspectorTab>('overview');
  const [sessions, setSessions] = useState<SessionSummary[] | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const agentId = agent?.id;

  const reload = useCallback(() => {
    if (!agentId) return;
    agentsApi
      .sessions(agentId)
      .then((list) => {
        setSessions(list);
        setProblem(null);
      })
      .catch((e: unknown) => setProblem(errorMessage(e)));
  }, [agentId]);

  // Outro agente: lista nova. Mesmo agente mudando de estado (subiu, caiu): a sessão
  // atual pode ter nascido ou acabado.
  // biome-ignore lint/correctness/useExhaustiveDependencies: roda a cada troca de agente
  useEffect(() => setSessions(null), [agentId]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `running` pede releitura
  useEffect(reload, [reload, isRunning(state)]);

  if (!agent) {
    return (
      <p className="px-3 py-2 text-caption text-muted">Selecione um agente para ver os detalhes.</p>
    );
  }

  return (
    <section aria-label={`Inspetor de @${agent.handle}`} className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 px-3 py-2.5">
        <span
          aria-hidden
          className="size-3 shrink-0 rounded-sm"
          style={{ background: `var(--agent-${agent.color})` }}
        />
        <div className="min-w-0">
          <h3 className="truncate text-heading text-primary">@{agent.handle}</h3>
          <p className="truncate text-caption text-muted">{agent.name}</p>
        </div>
      </div>
      {problem && (
        <p className="px-3 pb-2 text-caption text-failed" role="alert">
          {problem}
        </p>
      )}
      <Tabs
        value={tab}
        onValueChange={(value) => setTab(value as InspectorTab)}
        className="flex min-h-0 flex-1 flex-col"
      >
        <TabsList>
          <TabsTrigger value="overview">Visão</TabsTrigger>
          <TabsTrigger value="config">Config</TabsTrigger>
          <TabsTrigger value="logs">Logs</TabsTrigger>
        </TabsList>
        <TabsContent value="overview">
          <OverviewTab
            agent={agent}
            state={state}
            confidence={confidence}
            events={events}
            sessions={sessions}
            teamWorkdir={teamWorkdir}
          />
        </TabsContent>
        <TabsContent value="config">
          <AgentForm
            key={agent.id}
            teamId={teamId}
            agent={agent}
            siblings={siblings}
            running={isRunning(state)}
            active={tab === 'config'}
            onSaved={onSaved}
            compact
          />
        </TabsContent>
        <TabsContent value="logs">
          <LogsTab agent={agent} sessions={sessions} onReload={reload} />
        </TabsContent>
      </Tabs>
    </section>
  );
}
