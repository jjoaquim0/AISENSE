import { Archive, Check, X } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button, Dialog } from '@/components/ui';
import { MarkdownPreview } from '@/features/skills/MarkdownPreview';
import { errorMessage } from '@/features/teams/api';
import type { Actor } from '@/types/generated/Actor';
import type { AgentTag } from '@/types/generated/AgentTag';
import type { CardDetail } from '@/types/generated/CardDetail';
import type { CardPriority } from '@/types/generated/CardPriority';
import type { CardRef } from '@/types/generated/CardRef';
import type { Column } from '@/types/generated/Column';
import type { TeamId } from '@/types/generated/TeamId';
import { boardApi } from './api';
import { ago, PRIORITY_LABEL, shortId } from './boardModel';

const selectClass = 'h-7 rounded-md border border-strong bg-surface px-1.5 text-label text-primary';

/** Quem fez, como o quadro mostra: `@handle` com a cor do agente, você ou o AISENSE. */
export function actorOf(actor: Actor, agents: AgentTag[]): { label: string; color?: string } {
  if (actor.kind === 'human') return { label: '@voce' };
  if (actor.kind === 'system') return { label: 'AISENSE' };
  const agent = agents.find((a) => a.id === actor.agentId);
  return agent ? { label: `@${agent.handle}`, color: agent.color } : { label: 'agente removido' };
}

function Avatar({ label, color }: { label: string; color?: string }) {
  return (
    <span
      aria-hidden
      className="flex size-5 shrink-0 items-center justify-center rounded-full bg-hover text-caption text-primary"
      style={color ? { background: `var(--agent-${color})`, color: 'var(--accent-fg)' } : undefined}
    >
      {label.replace('@', '').slice(0, 1).toUpperCase()}
    </span>
  );
}

/**
 * Cartão aberto (F06-09): corpo em Markdown, checklist, comentários com avatar, dependências
 * navegáveis, links e histórico — e o gate de revisão (F06-11). Comentar daqui chega ao
 * responsável como mensagem, pelo barramento.
 */
export function CardDetailPanel({
  teamId,
  cardId,
  columns,
  agents,
  version,
  onOpenCard,
  onClose,
}: {
  teamId: TeamId;
  cardId: string;
  columns: Column[];
  agents: AgentTag[];
  /** Muda quando o quadro grava o cartão: relê. */
  version: number;
  onOpenCard: (id: string) => void;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<CardDetail | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [comment, setComment] = useState('');
  const [reason, setReason] = useState('');

  const reload = useCallback(
    () =>
      boardApi
        .show(teamId, cardId)
        .then(setDetail)
        .catch((e: unknown) => setProblem(errorMessage(e))),
    [teamId, cardId],
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: `version` marca gravação nova
  useEffect(() => {
    void reload();
  }, [reload, version]);

  const act = async (action: () => Promise<unknown>) => {
    setProblem(null);
    try {
      await action();
      await reload();
      return true;
    } catch (e: unknown) {
      setProblem(errorMessage(e));
      return false;
    }
  };

  if (!detail) {
    return null;
  }
  const card = detail.card;
  const inReview = detail.column.kind === 'review';

  const refs = (title: string, list: CardRef[]) =>
    list.length > 0 && (
      <section>
        <h4 className="mb-1 text-label text-secondary">{title}</h4>
        <ul className="flex flex-col gap-0.5">
          {list.map((r) => (
            <li key={r.id}>
              <button
                type="button"
                onClick={() => onOpenCard(r.id)}
                className="text-body text-accent hover:underline"
              >
                {shortId(r.id)} · {r.title}
              </button>{' '}
              <span className="text-caption text-muted">
                ({r.columnSlug}, {r.open ? 'aberto' : 'concluído'})
              </span>
            </li>
          ))}
        </ul>
      </section>
    );

  return (
    <Dialog
      open
      size="lg"
      onOpenChange={(open) => !open && onClose()}
      title={card.title}
      description={`${shortId(card.id)} · ${detail.column.name}${
        card.blockReason ? ` · bloqueado: ${card.blockReason}` : ''
      }`}
    >
      <div className="flex flex-col gap-4 select-text">
        <div className="flex flex-wrap items-center gap-2 text-label text-secondary">
          <label className="flex items-center gap-1">
            Coluna
            <select
              value={detail.column.slug}
              className={selectClass}
              onChange={(e) => void act(() => boardApi.move(teamId, card.id, e.target.value))}
            >
              {columns.map((c) => (
                <option key={c.id} value={c.slug}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-1">
            Responsável
            <select
              value={card.assigneeHandle ?? ''}
              className={selectClass}
              onChange={(e) =>
                void act(() => boardApi.update(teamId, card.id, { assignee: e.target.value }))
              }
            >
              <option value="">ninguém</option>
              {agents.map((a) => (
                <option key={a.id} value={a.handle}>
                  @{a.handle}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-1">
            Prioridade
            <select
              value={card.priority}
              className={selectClass}
              onChange={(e) =>
                void act(() =>
                  boardApi.update(teamId, card.id, { priority: e.target.value as CardPriority }),
                )
              }
            >
              {(Object.keys(PRIORITY_LABEL) as CardPriority[]).map((p) => (
                <option key={p} value={p}>
                  {PRIORITY_LABEL[p]}
                </option>
              ))}
            </select>
          </label>
          {card.labels.map((l) => (
            <span key={l} className="rounded-sm bg-hover px-1.5 py-0.5 text-caption">
              {l}
            </span>
          ))}
          <Button
            size="sm"
            variant="ghost"
            className="ml-auto"
            onClick={() => void act(() => boardApi.archive(teamId, card.id)).then(onClose)}
          >
            <Archive size={12} /> Arquivar
          </Button>
        </div>

        {problem && (
          <p role="alert" className="text-caption text-failed">
            {problem}
          </p>
        )}

        {inReview && (
          <section className="flex flex-col gap-2 rounded-md border border-subtle p-2">
            <h4 className="text-label text-secondary">Revisão</h4>
            <div className="flex items-center gap-2">
              <input
                aria-label="Motivo da rejeição"
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                placeholder="O que precisa mudar (obrigatório para rejeitar)"
                className="h-7 flex-1 rounded-md border border-strong bg-surface px-2 text-label text-primary"
              />
              <Button
                size="sm"
                disabled={!reason.trim()}
                onClick={() =>
                  void act(() => boardApi.reject(teamId, card.id, reason)).then(
                    (ok) => ok && setReason(''),
                  )
                }
              >
                <X size={12} /> Rejeitar
              </Button>
              <Button
                size="sm"
                variant="primary"
                onClick={() => void act(() => boardApi.approve(teamId, card.id))}
              >
                <Check size={12} /> Aprovar
              </Button>
            </div>
          </section>
        )}

        {card.body.trim() && (
          <section className="rounded-md border border-subtle px-3 py-1">
            <MarkdownPreview markdown={card.body} />
          </section>
        )}

        {card.checklist.length > 0 && (
          <section>
            <h4 className="mb-1 text-label text-secondary">Checklist</h4>
            <ul className="flex flex-col gap-0.5">
              {card.checklist.map((item, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: o item é endereçado pela posição, como na CLI
                <li key={i}>
                  <label className="flex items-center gap-2 text-body text-primary">
                    <input
                      type="checkbox"
                      checked={item.done}
                      onChange={(e) =>
                        void act(() => boardApi.check(teamId, card.id, i + 1, e.target.checked))
                      }
                    />
                    <span className={item.done ? 'text-muted line-through' : undefined}>
                      {item.text}
                    </span>
                  </label>
                </li>
              ))}
            </ul>
          </section>
        )}

        {refs('Depende de', detail.dependsOn)}
        {refs('Bloqueia', detail.dependents)}
        {refs('Subtarefas', detail.children)}

        {card.links.length > 0 && (
          <section>
            <h4 className="mb-1 text-label text-secondary">Links</h4>
            <ul className="flex flex-col gap-0.5 text-body">
              {card.links.map((l) => (
                <li key={`${l.kind}:${l.target}`}>
                  <span className="text-muted">{l.kind}</span>{' '}
                  <span className="font-mono">{l.target}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        <section>
          <h4 className="mb-1 text-label text-secondary">Comentários</h4>
          <ol className="flex flex-col gap-2">
            {detail.comments.map((c) => {
              const who = actorOf(c.author, agents);
              return (
                <li key={c.id} className="flex gap-2">
                  <Avatar label={who.label} color={who.color} />
                  <div className="min-w-0 flex-1">
                    <p className="text-caption text-muted">
                      <span className="text-primary">{who.label}</span> ·{' '}
                      {ago(Date.now(), c.createdAt)}
                    </p>
                    <p className="text-body whitespace-pre-wrap text-primary">{c.body}</p>
                  </div>
                </li>
              );
            })}
          </ol>
          <form
            className="mt-2 flex items-end gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (!comment.trim()) return;
              void act(() => boardApi.comment(teamId, card.id, comment)).then(
                (ok) => ok && setComment(''),
              );
            }}
          >
            <textarea
              aria-label="Comentário"
              rows={2}
              value={comment}
              placeholder="Comente — o responsável recebe como mensagem"
              onChange={(e) => setComment(e.target.value)}
              className="min-h-8 flex-1 rounded-md border border-strong bg-surface px-2.5 py-1.5 text-body text-primary"
            />
            <Button type="submit" disabled={!comment.trim()}>
              Comentar
            </Button>
          </form>
        </section>

        <section>
          <h4 className="mb-1 text-label text-secondary">Histórico</h4>
          <ol className="flex flex-col gap-0.5 text-caption text-muted">
            {detail.activity.map((a) => {
              const who = actorOf(a.actor, agents);
              const from = typeof a.detail.from === 'string' ? a.detail.from : null;
              const to = typeof a.detail.to === 'string' ? a.detail.to : null;
              return (
                <li key={a.id}>
                  {ago(Date.now(), a.createdAt)} ·{' '}
                  <span className="text-secondary">{who.label}</span> {a.action}
                  {from && to ? ` ${from} → ${to}` : ''}
                </li>
              );
            })}
          </ol>
        </section>
      </div>
    </Dialog>
  );
}
