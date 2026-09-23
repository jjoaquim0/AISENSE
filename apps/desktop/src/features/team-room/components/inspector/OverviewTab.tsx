import { Folder } from 'lucide-react';
import { useEffect, useState } from 'react';
import { StatusDot } from '@/components/ui';
import type { Agent } from '@/types/generated/Agent';
import type { AgentState } from '@/types/generated/AgentState';
import type { SessionSummary } from '@/types/generated/SessionSummary';
import type { StateConfidence } from '@/types/generated/StateConfidence';
import type { StateEvent } from '../../hooks/useLiveStates';
import { isRunning } from '../../sidebar';
import { formatDuration } from '../../transcriptSearch';

interface OverviewTabProps {
  agent: Agent;
  state: AgentState;
  confidence?: StateConfidence;
  events: StateEvent[];
  sessions: SessionSummary[] | null;
  teamWorkdir: string;
}

const time = new Intl.DateTimeFormat('pt-BR', { timeStyle: 'medium' });

/**
 * Aba Visão (docs/09, T6): estado, tempo ativo, PID e os últimos eventos. Mensagens
 * trocadas e tarefas entram com o barramento (Fase 05) e o quadro (Fase 06).
 */
export function OverviewTab({
  agent,
  state,
  confidence,
  events,
  sessions,
  teamWorkdir,
}: OverviewTabProps) {
  const running = isRunning(state);
  const current = running ? sessions?.[0] : undefined;
  const now = useNow(running);

  return (
    <div className="flex flex-col gap-4">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-caption">
        <dt className="text-muted">Estado</dt>
        <dd>
          <StatusDot state={state} confidence={confidence} withLabel />
        </dd>
        <dt className="text-muted">Tempo ativo</dt>
        <dd className="text-secondary tabular-nums">
          {current ? formatDuration(now - current.startedAt) : '—'}
        </dd>
        <dt className="text-muted">PID</dt>
        <dd className="font-mono text-secondary">{current?.pid ?? '—'}</dd>
        <dt className="text-muted">Sessões</dt>
        <dd className="text-secondary tabular-nums">{sessions?.length ?? '…'}</dd>
        <dt className="text-muted">Runtime</dt>
        <dd className="text-secondary">
          {agent.adapterId}
          {agent.model && ` · ${agent.model}`}
        </dd>
        <dt className="text-muted">Pasta</dt>
        <dd className="flex min-w-0 items-center gap-1 text-secondary">
          <Folder size={11} className="shrink-0" />
          <span className="truncate font-mono" title={agent.workdir ?? teamWorkdir}>
            {agent.workdir ?? teamWorkdir}
          </span>
        </dd>
        {agent.role && (
          <>
            <dt className="text-muted">Papel</dt>
            <dd className="line-clamp-4 text-secondary">{agent.role}</dd>
          </>
        )}
      </dl>

      <section aria-label="Últimos eventos">
        <h4 className="pb-1 text-caption tracking-[0.02em] text-muted uppercase">
          Últimos eventos
        </h4>
        {events.length === 0 ? (
          <p className="text-caption text-muted">
            Nenhuma mudança de estado desde que a janela abriu.
          </p>
        ) : (
          <ol className="flex flex-col gap-1">
            {events.map((event) => (
              <li
                key={`${event.at}-${event.state}`}
                className="flex items-center justify-between gap-2 text-caption"
              >
                <StatusDot state={event.state} confidence={event.confidence} withLabel />
                <time
                  className="text-muted tabular-nums"
                  dateTime={new Date(event.at).toISOString()}
                >
                  {time.format(new Date(event.at))}
                </time>
              </li>
            ))}
          </ol>
        )}
      </section>
    </div>
  );
}

/** Relógio de 1 s só enquanto o agente roda: parado, o tempo ativo não muda. */
function useNow(ticking: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    if (!ticking) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [ticking]);
  return now;
}
