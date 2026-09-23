import { Folder } from 'lucide-react';
import { StatusDot } from '@/components/ui';
import type { Agent } from '@/types/generated/Agent';
import type { AgentState } from '@/types/generated/AgentState';
import type { StateConfidence } from '@/types/generated/StateConfidence';

interface AgentInspectorProps {
  agent: Agent | null;
  state: AgentState;
  confidence?: StateConfidence;
  /** Diretório da equipe, quando o agente não tem um próprio. */
  teamWorkdir: string;
}

/**
 * Cabeçalho do inspetor (docs/09, T6) para o agente selecionado. As abas Visão, Config
 * e Logs entram na F03-09; por ora é o que a sidebar abre com duplo clique.
 */
export function AgentInspector({ agent, state, confidence, teamWorkdir }: AgentInspectorProps) {
  if (!agent) {
    return (
      <p className="px-3 py-2 text-caption text-muted">Selecione um agente para ver os detalhes.</p>
    );
  }
  return (
    <section aria-label={`Inspetor de @${agent.handle}`} className="flex flex-col gap-3 px-3 py-3">
      <div className="flex items-center gap-2">
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
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-caption">
        <dt className="text-muted">Estado</dt>
        <dd>
          <StatusDot state={state} confidence={confidence} withLabel />
        </dd>
        <dt className="text-muted">Runtime</dt>
        <dd className="text-secondary">
          {agent.adapterId}
          {agent.model && ` · ${agent.model}`}
        </dd>
        <dt className="text-muted">Pasta</dt>
        <dd className="flex min-w-0 items-center gap-1 text-secondary">
          <Folder size={11} className="shrink-0" />
          <span className="truncate font-mono">{agent.workdir ?? teamWorkdir}</span>
        </dd>
        {agent.role && (
          <>
            <dt className="text-muted">Papel</dt>
            <dd className="line-clamp-4 text-secondary">{agent.role}</dd>
          </>
        )}
      </dl>
    </section>
  );
}
