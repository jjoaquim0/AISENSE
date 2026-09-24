import { Clock, Send, ShieldAlert } from 'lucide-react';
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Button } from '@/components/ui';
import { busApi, onBusMessage, onBusRead } from '@/features/bus/api';
import { errorMessage } from '@/features/teams/api';
import { cn } from '@/lib/cn';
import type { Agent } from '@/types/generated/Agent';
import type { MessageView } from '@/types/generated/MessageView';
import type { TeamId } from '@/types/generated/TeamId';
import {
  applyFilter,
  askRemaining,
  destinations,
  formatCountdown,
  mergeMessages,
  nearBottom,
  receiptLabel,
  type TimelineFilter,
} from './timelineModel';
import { useVirtualList } from './useVirtualList';

/** Quantas mensagens cada página traz. */
export const TIMELINE_PAGE = 200;

const time = new Intl.DateTimeFormat('pt-BR', { timeStyle: 'short' });

interface TimelineViewProps {
  teamId: TeamId;
  agents: Agent[];
}

/**
 * Linha do tempo da equipe (docs/09, T4.4): toda a conversa, com cor de quem mandou,
 * recibos, perguntas pendentes com contagem regressiva, avisos do sistema com ação,
 * filtros e o compositor do humano (`@voce`).
 *
 * Lista virtual (`useVirtualList`): só as mensagens perto da tela existem no DOM, então 10
 * mil rolam leves. Mensagem nova só rola a tela se você já estava no fim — lendo o
 * histórico, nada pula.
 */
export function TimelineView({ teamId, agents }: TimelineViewProps) {
  const [messages, setMessages] = useState<MessageView[]>([]);
  const [hasOlder, setHasOlder] = useState(true);
  const [problem, setProblem] = useState<string | null>(null);
  const [filter, setFilter] = useState<TimelineFilter>({ agent: null, onlyConversation: false });
  const [now, setNow] = useState(() => Date.now());
  const list = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);
  const prepending = useRef<number | null>(null);

  const colorOf = useMemo(() => {
    const map = new Map(agents.map((a) => [`@${a.handle}`, a.color]));
    return (label: string) => map.get(label);
  }, [agents]);

  const loadLatest = useCallback(() => {
    busApi
      .timeline(teamId, undefined, TIMELINE_PAGE)
      .then((page) => {
        setMessages((current) => mergeMessages(current, page));
        if (page.length < TIMELINE_PAGE) setHasOlder(false);
      })
      .catch((e: unknown) => setProblem(errorMessage(e)));
  }, [teamId]);

  useEffect(() => {
    setMessages([]);
    setHasOlder(true);
    stickToBottom.current = true;
    loadLatest();
    const offMessage = onBusMessage((event) => {
      if (event.teamId !== teamId) return;
      const el = list.current;
      stickToBottom.current = el ? nearBottom(el) : true;
      setMessages((current) => mergeMessages(current, [event.message]));
    });
    // Recibos mudam quando alguém lê: relê a página mais recente.
    const offRead = onBusRead(() => loadLatest());
    return () => {
      void offMessage.then((stop) => stop());
      void offRead.then((stop) => stop());
    };
  }, [teamId, loadLatest]);

  const loadOlder = () => {
    const oldest = messages[0];
    if (!oldest) return;
    prepending.current = list.current?.scrollHeight ?? null;
    stickToBottom.current = false;
    busApi
      .timeline(teamId, oldest.id, TIMELINE_PAGE)
      .then((page) => {
        if (page.length < TIMELINE_PAGE) setHasOlder(false);
        setMessages((current) => mergeMessages(current, page));
      })
      .catch((e: unknown) => setProblem(errorMessage(e)));
  };

  const visible = useMemo(() => applyFilter(messages, filter), [messages, filter]);
  const keys = useMemo(() => visible.map((m) => m.id), [visible]);
  const rows = useVirtualList(keys, list);
  // Depois de pintar: fim da lista se era para acompanhar; posição mantida ao trazer
  // mensagens antigas para cima.
  // biome-ignore lint/correctness/useExhaustiveDependencies: roda a cada lista nova e a cada altura medida
  useLayoutEffect(() => {
    const el = list.current;
    if (!el) return;
    if (prepending.current !== null) {
      el.scrollTop += el.scrollHeight - prepending.current;
      prepending.current = null;
    } else if (stickToBottom.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [messages, rows.total]);

  const pendingAsks = messages.some((m) => askRemaining(m, messages, now) !== null);
  useEffect(() => {
    if (!pendingAsks) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [pendingAsks]);

  const handles = agents.map((a) => a.handle);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center gap-2 border-b border-subtle px-3 py-1.5 text-caption">
        <label className="flex items-center gap-1 text-muted">
          Agente
          <select
            value={filter.agent ?? ''}
            onChange={(e) => setFilter((f) => ({ ...f, agent: e.target.value || null }))}
            className="rounded-sm border border-subtle bg-surface px-1 py-0.5 text-primary"
          >
            <option value="">todos</option>
            {handles.map((h) => (
              <option key={h} value={`@${h}`}>
                @{h}
              </option>
            ))}
            <option value="@voce">@voce</option>
          </select>
        </label>
        <label className="flex items-center gap-1 text-muted">
          <input
            type="checkbox"
            checked={filter.onlyConversation}
            onChange={(e) => setFilter((f) => ({ ...f, onlyConversation: e.target.checked }))}
          />
          só conversa
        </label>
        <span className="ml-auto text-muted tabular-nums">{visible.length} mensagens</span>
      </div>

      <div
        ref={list}
        role="log"
        aria-label="Linha do tempo da equipe"
        aria-live="polite"
        onScroll={() => {
          const el = list.current;
          if (el) stickToBottom.current = nearBottom(el);
        }}
        className="min-h-0 flex-1 overflow-y-auto px-3 py-2"
      >
        {hasOlder && messages.length > 0 && (
          <div className="flex justify-center pb-2">
            <Button size="sm" variant="ghost" onClick={loadOlder}>
              Carregar anteriores
            </Button>
          </div>
        )}
        {messages.length === 0 && !problem && (
          <p className="py-8 text-center text-caption text-muted">
            Nenhuma mensagem ainda. Os agentes conversam com `aisense send` — ou comece você, aqui
            embaixo.
          </p>
        )}
        {/* Janela virtual: só o que está perto da tela existe no DOM. */}
        <ol style={{ paddingTop: rows.before, paddingBottom: rows.after }}>
          {visible.slice(rows.first, rows.last + 1).map((m) => (
            <MessageRow
              key={m.id}
              message={m}
              measure={rows.measure}
              color={colorOf(m.from)}
              remaining={askRemaining(m, messages, now)}
              teamId={teamId}
            />
          ))}
        </ol>
      </div>
      {problem && (
        <p role="alert" className="border-t border-subtle px-3 py-1 text-caption text-failed">
          {problem}
        </p>
      )}
      <Composer teamId={teamId} options={destinations(handles, messages)} />
    </div>
  );
}

function MessageRow({
  message,
  measure,
  color,
  remaining,
  teamId,
}: {
  message: MessageView;
  measure: (el: HTMLElement | null) => void;
  color: string | undefined;
  remaining: number | null;
  teamId: TeamId;
}) {
  const system = message.kind === 'system';
  const receipt = receiptLabel(message);
  const paused =
    system && message.subject === 'guarda anti-laço' && message.body.includes('pausadas');
  return (
    <li ref={measure} data-key={message.id} className="pb-1.5">
      <div
        style={{ borderLeftColor: color ? `var(--agent-${color})` : undefined }}
        className={cn(
          'rounded-md border border-l-[3px] border-subtle bg-surface px-2.5 py-1.5',
          system && 'border-l-awaiting bg-base',
          message.from === '@voce' && 'border-l-accent',
        )}
      >
        <div className="flex items-baseline gap-1.5 text-caption">
          {system && <ShieldAlert size={12} className="self-center text-awaiting" />}
          <span className="font-medium text-primary">{message.from}</span>
          <span className="text-muted">→ {message.to}</span>
          {message.kind === 'request' && <span className="text-awaiting">pergunta</span>}
          {message.kind === 'response' && <span className="text-idle">resposta</span>}
          {message.kind === 'event' && <span className="text-muted">registro</span>}
          {remaining !== null && (
            <span
              className="flex items-center gap-0.5 text-awaiting tabular-nums"
              title="Tempo que falta para a pergunta expirar"
            >
              <Clock size={11} /> {formatCountdown(remaining)}
            </span>
          )}
          <time className="ml-auto text-muted" dateTime={new Date(message.createdAt).toISOString()}>
            {time.format(new Date(message.createdAt))}
          </time>
        </div>
        <p className="text-body whitespace-pre-wrap text-primary">{message.body}</p>
        {(receipt || paused) && (
          <div className="mt-0.5 flex items-center gap-2 text-caption text-muted">
            {receipt && <span>{receipt}</span>}
            {paused && (
              <Button size="sm" onClick={() => void busApi.resume(teamId)}>
                Continuar
              </Button>
            )}
          </div>
        )}
      </div>
    </li>
  );
}

/** Você (`@voce`) no mesmo fluxo que os agentes. Enter manda; Shift+Enter quebra linha. */
function Composer({ teamId, options }: { teamId: TeamId; options: string[] }) {
  const [to, setTo] = useState('@all');
  const [body, setBody] = useState('');
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const send = async () => {
    if (!body.trim()) return;
    setBusy(true);
    setProblem(null);
    try {
      await busApi.send(teamId, [to], body);
      setBody('');
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      aria-label="Mandar mensagem"
      onSubmit={(e) => {
        e.preventDefault();
        void send();
      }}
      className="flex flex-col gap-1 border-t border-subtle px-3 py-2"
    >
      <div className="flex items-end gap-2">
        <select
          aria-label="Para"
          value={to}
          onChange={(e) => setTo(e.target.value)}
          className="h-8 rounded-md border border-strong bg-surface px-1.5 text-body text-primary"
        >
          {options.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
        <textarea
          aria-label="Mensagem"
          rows={1}
          value={body}
          placeholder="Escreva para a equipe…"
          onChange={(e) => setBody(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          className="max-h-32 min-h-8 flex-1 resize-y rounded-md border border-strong bg-surface px-2.5 py-1.5 text-body text-primary"
        />
        <Button type="submit" variant="primary" disabled={busy || !body.trim()}>
          <Send size={13} /> Enviar
        </Button>
      </div>
      {problem && (
        <p role="alert" className="text-caption text-failed">
          {problem}
        </p>
      )}
    </form>
  );
}
