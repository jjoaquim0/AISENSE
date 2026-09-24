import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { ChevronDown, ChevronUp, Download, RefreshCw, Search } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Button, IconButton, Tooltip } from '@/components/ui';
import { agentsApi } from '@/features/agents/api';
import { errorMessage } from '@/features/teams/api';
import { cn } from '@/lib/cn';
import type { Agent } from '@/types/generated/Agent';
import type { SessionSummary } from '@/types/generated/SessionSummary';
import type { Transcript } from '@/types/generated/Transcript';
import { exportFileName, findMatches, MAX_MATCHES, splitByMatches } from '../../transcriptSearch';

interface LogsTabProps {
  agent: Agent;
  sessions: SessionSummary[] | null;
  onReload: () => void;
}

const dateTime = new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short', timeStyle: 'short' });

/** Rótulo de uma sessão no seletor: quando começou e como terminou. */
export function sessionLabel(session: SessionSummary, index: number): string {
  const when = dateTime.format(new Date(session.startedAt));
  const end =
    session.endedAt === null
      ? index === 0
        ? 'em andamento'
        : 'sem registro de fim'
      : session.exitCode === 0
        ? 'saiu bem'
        : `saiu com código ${session.exitCode ?? '?'}`;
  return `${when} · ${end}${session.available ? '' : ' · transcrição indisponível'}`;
}

/**
 * Aba Logs do inspetor (docs/09, T6): a transcrição de uma sessão, com busca e
 * exportação. O texto vem do core já sem os códigos de terminal; aqui só se busca.
 */
export function LogsTab({ agent, sessions, onReload }: LogsTabProps) {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [current, setCurrent] = useState(0);
  const [loading, setLoading] = useState(false);
  const [version, setVersion] = useState(0);
  const view = useRef<HTMLPreElement>(null);

  // Sem escolha, a sessão mais recente; escolha que sumiu (podada) também volta a ela.
  const selected = sessions?.find((s) => s.id === sessionId) ?? sessions?.[0] ?? null;

  // biome-ignore lint/correctness/useExhaustiveDependencies: `version` pede releitura
  useEffect(() => {
    setTranscript(null);
    setProblem(null);
    if (!selected) return;
    if (!selected.available) {
      setProblem('O trecho desta sessão não está mais no log.');
      return;
    }
    let live = true;
    setLoading(true);
    agentsApi
      .transcript(agent.id, selected.id)
      .then((t) => live && setTranscript(t))
      .catch((e: unknown) => live && setProblem(errorMessage(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [agent.id, selected?.id, selected?.available, version]);

  const text = transcript?.text ?? '';
  const matches = useMemo(() => findMatches(text, query), [text, query]);
  const parts = useMemo(() => splitByMatches(text, matches), [text, matches]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: busca ou texto novos voltam à primeira
  useEffect(() => setCurrent(0), [query, text]);
  // Leva a ocorrência atual para a vista.
  useEffect(() => {
    if (matches.length === 0) return;
    view.current?.querySelector(`[data-match="${current}"]`)?.scrollIntoView({ block: 'center' });
  }, [current, matches]);

  const step = (delta: number) =>
    matches.length > 0 && setCurrent((c) => (c + delta + matches.length) % matches.length);

  const exportIt = async () => {
    if (!selected) return;
    setNotice(null);
    const path = await saveDialog({
      title: 'Exportar transcrição',
      defaultPath: exportFileName(agent.handle, selected.startedAt),
      filters: [{ name: 'Texto', extensions: ['txt', 'log'] }],
    });
    if (typeof path !== 'string') return;
    try {
      const bytes = await agentsApi.exportTranscript(agent.id, selected.id, path);
      setNotice(`Transcrição exportada (${Math.max(1, Math.round(bytes / 1024))} KB).`);
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  if (sessions && sessions.length === 0) {
    return <p className="text-caption text-muted">Nenhuma sessão ainda: inicie o agente.</p>;
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div className="flex items-center gap-1">
        <label className="sr-only" htmlFor="logs-session">
          Sessão
        </label>
        <select
          id="logs-session"
          value={selected?.id ?? ''}
          onChange={(e) => setSessionId(e.target.value)}
          className="h-7 min-w-0 flex-1 rounded-md border border-strong bg-surface px-1.5 text-caption text-primary"
        >
          {sessions?.map((s, i) => (
            <option key={s.id} value={s.id}>
              {sessionLabel(s, i)}
            </option>
          ))}
        </select>
        <Tooltip content="Recarregar">
          <IconButton
            label="Recarregar a transcrição"
            size="sm"
            onClick={() => {
              onReload();
              setVersion((v) => v + 1);
            }}
          >
            <RefreshCw size={12} className={cn(loading && 'animate-spin')} />
          </IconButton>
        </Tooltip>
      </div>

      <div className="flex items-center gap-1">
        <div className="relative min-w-0 flex-1">
          <Search
            size={12}
            className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-muted"
          />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                step(e.shiftKey ? -1 : 1);
              }
            }}
            placeholder="Buscar na transcrição"
            aria-label="Buscar na transcrição"
            className="h-7 w-full rounded-md border border-strong bg-surface pr-2 pl-6 text-caption text-primary placeholder:text-muted focus:border-emphasis"
          />
        </div>
        <span
          className="w-16 shrink-0 text-right text-caption text-muted tabular-nums"
          aria-live="polite"
        >
          {query.trim()
            ? matches.length === 0
              ? 'nada'
              : `${current + 1}/${matches.length}${matches.length >= MAX_MATCHES ? '+' : ''}`
            : ''}
        </span>
        <IconButton label="Ocorrência anterior" size="sm" onClick={() => step(-1)}>
          <ChevronUp size={12} />
        </IconButton>
        <IconButton label="Próxima ocorrência" size="sm" onClick={() => step(1)}>
          <ChevronDown size={12} />
        </IconButton>
      </div>

      {transcript?.truncated && (
        <p className="text-caption text-muted">
          Mostrando só o final. Exporte para ter a sessão inteira.
        </p>
      )}
      {problem && (
        <p className="text-caption text-failed" role="alert">
          {problem}
        </p>
      )}

      <section aria-label="Transcrição da sessão" className="flex min-h-0 flex-1">
        <pre
          ref={view}
          // biome-ignore lint/a11y/noNoninteractiveTabindex: área rolável precisa ser alcançável pelo teclado
          tabIndex={0}
          className="min-h-0 flex-1 overflow-auto rounded-md border border-subtle bg-terminal p-2 font-mono text-[11px] leading-snug whitespace-pre-wrap break-words text-secondary"
        >
          {transcript && text.length === 0 && <span className="text-muted">Sessão sem saída.</span>}
          {parts.map((part, i) =>
            part.match === null ? (
              // biome-ignore lint/suspicious/noArrayIndexKey: pedaços posicionais, recalculados juntos
              <span key={i}>{part.text}</span>
            ) : (
              <mark
                // biome-ignore lint/suspicious/noArrayIndexKey: idem
                key={i}
                data-match={part.match}
                className={cn(
                  'rounded-sm text-primary',
                  part.match === current ? 'bg-accent text-accent-fg' : 'bg-active',
                )}
              >
                {part.text}
              </mark>
            ),
          )}
        </pre>
      </section>

      <div className="flex items-center justify-between gap-2">
        <span className="truncate text-caption text-muted" role="status">
          {notice}
        </span>
        <Button size="sm" disabled={!selected?.available} onClick={() => void exportIt()}>
          <Download size={12} /> Exportar
        </Button>
      </div>
    </div>
  );
}
