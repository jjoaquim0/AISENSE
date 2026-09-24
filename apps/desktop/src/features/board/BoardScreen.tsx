import {
  AlertTriangle,
  ArrowUp,
  CheckSquare,
  Columns3,
  Link2,
  MessageSquare,
  Plus,
  Workflow,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Dialog, Input } from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import { cn } from '@/lib/cn';
import type { BoardView } from '@/types/generated/BoardView';
import type { CardView } from '@/types/generated/CardView';
import type { Column } from '@/types/generated/Column';
import type { TeamId } from '@/types/generated/TeamId';
import { AutomationsEditor } from './AutomationsEditor';
import { boardApi, onBoardChanged } from './api';
import {
  applyFilter,
  type BoardFilter,
  byColumn,
  changedCards,
  countLabel,
  HIGHLIGHT_MS,
  labelsOf,
  lastSeen,
  markSeen,
  moveLocally,
  PRIORITY_LABEL,
} from './boardModel';
import { CardDetailPanel } from './CardDetailPanel';
import { ColumnsEditor } from './ColumnsEditor';
import { NewCardDialog } from './NewCardDialog';

const DRAG_TYPE = 'application/x-aisense-card';

/**
 * Quadro da equipe (T8, docs/13). Arrastar chama a mesma operação da CLI: se o core recusar
 * (coluna cheia, bloqueio sem motivo, gate), o cartão volta e o erro aparece igual ao da CLI.
 */
export function BoardScreen({ teamId }: { teamId: TeamId }) {
  const [view, setView] = useState<BoardView | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [warning, setWarning] = useState<string | null>(null);
  const [filter, setFilter] = useState<BoardFilter>({ assignee: null, label: null });
  const [highlight, setHighlight] = useState<Set<string>>(new Set());
  const [changed, setChanged] = useState(0);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [editor, setEditor] = useState<'columns' | 'automations' | null>(null);
  const [blocking, setBlocking] = useState<{ card: CardView; column: Column } | null>(null);
  const viewRef = useRef<BoardView | null>(null);
  viewRef.current = view;

  const reload = useCallback(
    () =>
      boardApi
        .get(teamId)
        .then(setView)
        .catch((e: unknown) => setProblem(errorMessage(e))),
    [teamId],
  );

  useEffect(() => {
    void reload();
    const timers = new Set<ReturnType<typeof setTimeout>>();
    const off = onBoardChanged((event) => {
      if (event.teamId !== teamId) return;
      void reload();
      const id = event.cardId;
      if (!id) return;
      setHighlight((h) => new Set(h).add(id));
      const timer = setTimeout(() => {
        timers.delete(timer);
        setHighlight((h) => {
          const next = new Set(h);
          next.delete(id);
          return next;
        });
      }, HIGHLIGHT_MS);
      timers.add(timer);
    });
    return () => {
      void off.then((stop) => stop());
      for (const t of timers) clearTimeout(t);
    };
  }, [teamId, reload]);

  // "N cartões mudaram desde que você saiu": conta ao abrir, marca a visita ao sair.
  useEffect(() => {
    const now = Date.now();
    const since = lastSeen(teamId, now);
    if (since < now) {
      boardApi
        .changes(teamId, since)
        .then((activity) => setChanged(changedCards(activity)))
        .catch(() => {});
    }
    markSeen(teamId, now);
    return () => markSeen(teamId, Date.now());
  }, [teamId]);

  const move = async (card: CardView, column: Column, reason?: string) => {
    const before = viewRef.current;
    if (!before || card.columnId === column.id) return;
    setProblem(null);
    setWarning(null);
    setView(moveLocally(before, card.id, column));
    try {
      const moved = await boardApi.move(teamId, card.id, column.slug, reason);
      if (moved.warnings.length > 0) setWarning(moved.warnings.join(' · '));
      await reload();
    } catch (e: unknown) {
      // Mesma mensagem da CLI; o cartão volta para onde estava.
      setView(before);
      setProblem(errorMessage(e));
    }
  };

  const drop = (column: Column, cardId: string) => {
    const card = viewRef.current?.cards.find((c) => c.id === cardId);
    if (!card || card.columnId === column.id) return;
    if (column.kind === 'blocked' && !card.blockReason) {
      setBlocking({ card, column });
      return;
    }
    void move(card, column);
  };

  const visible = useMemo(() => (view ? applyFilter(view.cards, filter) : []), [view, filter]);
  const columns = useMemo(() => (view ? byColumn(view, visible) : new Map()), [view, visible]);
  const colorOf = useMemo(() => {
    const map = new Map((view?.agents ?? []).map((a) => [a.handle, a.color]));
    return (handle: string | null) => (handle ? map.get(handle) : undefined);
  }, [view]);

  if (!view) {
    return problem ? (
      <p role="alert" className="p-4 text-caption text-failed">
        {problem}
      </p>
    ) : null;
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-subtle px-3 py-1.5 text-caption">
        <Button size="sm" variant="primary" onClick={() => setAdding(true)}>
          <Plus size={12} /> Cartão
        </Button>
        <label className="flex items-center gap-1 text-muted">
          Responsável
          <select
            value={filter.assignee ?? ''}
            onChange={(e) => setFilter((f) => ({ ...f, assignee: e.target.value || null }))}
            className="rounded-sm border border-subtle bg-surface px-1 py-0.5 text-primary"
          >
            <option value="">todos</option>
            <option value="none">sem responsável</option>
            {view.agents.map((a) => (
              <option key={a.id} value={a.handle}>
                @{a.handle}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1 text-muted">
          Label
          <select
            value={filter.label ?? ''}
            onChange={(e) => setFilter((f) => ({ ...f, label: e.target.value || null }))}
            className="rounded-sm border border-subtle bg-surface px-1 py-0.5 text-primary"
          >
            <option value="">todas</option>
            {labelsOf(view.cards).map((l) => (
              <option key={l} value={l}>
                {l}
              </option>
            ))}
          </select>
        </label>
        {changed > 0 && (
          <button
            type="button"
            className="rounded-sm bg-active px-1.5 py-0.5 text-primary"
            onClick={() => setChanged(0)}
            title="Os cartões realçados mudaram enquanto você estava fora"
          >
            {changed === 1
              ? '1 cartão mudou desde que você saiu'
              : `${changed} cartões mudaram desde que você saiu`}
          </button>
        )}
        <span className="ml-auto flex gap-1">
          <Button size="sm" variant="ghost" onClick={() => setEditor('columns')}>
            <Columns3 size={12} /> Colunas
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setEditor('automations')}>
            <Workflow size={12} /> Automações
          </Button>
        </span>
      </div>
      {(problem || warning) && (
        <div className="border-b border-subtle px-3 py-1 text-caption">
          {problem && (
            <p role="alert" className="text-failed">
              {problem}
            </p>
          )}
          {warning && <p className="text-awaiting">{warning}</p>}
        </div>
      )}

      {/* Focável: com o quadro vazio não há cartão para levar o teclado à rolagem (F08-02). */}
      <section
        aria-label="Colunas do quadro"
        // biome-ignore lint/a11y/noNoninteractiveTabindex: região rolável precisa de foco para o teclado
        tabIndex={0}
        className="flex min-h-0 flex-1 gap-2 overflow-x-auto p-2 outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
      >
        {view.columns.map((column) => {
          const cards: CardView[] = columns.get(column.id) ?? [];
          const all = view.cards.filter((c) => c.columnId === column.id).length;
          const full = column.wipLimit !== null && all >= column.wipLimit;
          return (
            <section
              key={column.id}
              aria-label={column.name}
              data-column={column.slug}
              onDragOver={(e) => {
                if (e.dataTransfer.types.includes(DRAG_TYPE)) e.preventDefault();
              }}
              onDrop={(e) => {
                e.preventDefault();
                drop(column, e.dataTransfer.getData(DRAG_TYPE));
              }}
              className="flex w-60 shrink-0 flex-col rounded-lg border border-subtle bg-base"
            >
              <header className="flex items-baseline gap-1.5 px-2.5 pt-2 pb-1">
                <h3 className="text-label text-primary uppercase">{column.name}</h3>
                <span
                  className={cn('text-caption tabular-nums', full ? 'text-awaiting' : 'text-muted')}
                >
                  ({countLabel(column, all)})
                </span>
              </header>
              <ol className="flex min-h-12 flex-1 flex-col gap-1.5 overflow-y-auto px-2 pb-2">
                {cards.map((card) => (
                  <li key={card.id}>
                    <CardTile
                      card={card}
                      color={colorOf(card.assigneeHandle)}
                      highlighted={highlight.has(card.id)}
                      onOpen={() => setDetailId(card.id)}
                    />
                  </li>
                ))}
              </ol>
            </section>
          );
        })}
      </section>

      {detailId && (
        <CardDetailPanel
          teamId={teamId}
          cardId={detailId}
          columns={view.columns}
          agents={view.agents}
          version={view.cards.find((c) => c.id === detailId)?.version ?? 0}
          onOpenCard={setDetailId}
          onClose={() => setDetailId(null)}
        />
      )}
      <NewCardDialog
        open={adding}
        teamId={teamId}
        columns={view.columns}
        agents={view.agents}
        onOpenChange={setAdding}
        onCreated={() => void reload()}
      />
      {editor === 'columns' && (
        <ColumnsEditor
          teamId={teamId}
          view={view}
          onClose={() => setEditor(null)}
          onSaved={() => void reload()}
        />
      )}
      {editor === 'automations' && (
        <AutomationsEditor
          teamId={teamId}
          view={view}
          onClose={() => setEditor(null)}
          onSaved={() => void reload()}
        />
      )}
      {blocking && (
        <BlockReasonDialog
          column={blocking.column}
          onCancel={() => setBlocking(null)}
          onConfirm={(reason) => {
            const { card, column } = blocking;
            setBlocking(null);
            void move(card, column, reason);
          }}
        />
      )}
    </div>
  );
}

export function CardTile({
  card,
  color,
  highlighted,
  onOpen,
}: {
  card: CardView;
  color: string | undefined;
  highlighted: boolean;
  onOpen: () => void;
}) {
  const done = card.checklist.filter((i) => i.done).length;
  return (
    <button
      type="button"
      draggable
      data-card={card.id}
      onDragStart={(e) => {
        e.dataTransfer.setData(DRAG_TYPE, card.id);
        e.dataTransfer.effectAllowed = 'move';
      }}
      onClick={onOpen}
      style={{ borderLeftColor: color ? `var(--agent-${color})` : undefined }}
      className={cn(
        'flex w-full flex-col gap-1 rounded-md border border-l-[3px] border-subtle bg-surface px-2 py-1.5 text-left',
        'transition-colors duration-100 ease-out hover:bg-hover',
        highlighted && 'bg-active',
      )}
    >
      <span className="line-clamp-2 text-body text-primary">{card.title}</span>
      <span className="flex flex-wrap items-center gap-2 text-caption text-muted">
        {(card.priority === 'high' || card.priority === 'urgent') && (
          <span
            className={cn('flex items-center', card.priority === 'urgent' && 'text-failed')}
            title={`Prioridade ${PRIORITY_LABEL[card.priority]}`}
          >
            <ArrowUp size={11} />
            {PRIORITY_LABEL[card.priority]}
          </span>
        )}
        {card.blockedBy.length > 0 && (
          <span className="flex items-center gap-0.5" title="Dependências abertas">
            <Link2 size={11} />
            {card.blockedBy.length}
          </span>
        )}
        {card.checklist.length > 0 && (
          <span className="flex items-center gap-0.5 tabular-nums" title="Checklist">
            <CheckSquare size={11} />
            {done}/{card.checklist.length}
          </span>
        )}
        {card.comments > 0 && (
          <span className="flex items-center gap-0.5" title="Comentários">
            <MessageSquare size={11} />
            {card.comments}
          </span>
        )}
        <span className="ml-auto">
          {card.assigneeHandle ? `@${card.assigneeHandle}` : 'sem responsável'}
        </span>
      </span>
      {card.blockReason && (
        <span className="flex items-start gap-1 text-caption text-awaiting">
          <AlertTriangle size={11} className="mt-px shrink-0" />
          <span className="line-clamp-2">{card.blockReason}</span>
        </span>
      )}
    </button>
  );
}

/** Bloquear exige motivo (docs/13) — a UI pergunta antes, em vez de levar o erro. */
function BlockReasonDialog({
  column,
  onCancel,
  onConfirm,
}: {
  column: Column;
  onCancel: () => void;
  onConfirm: (reason: string) => void;
}) {
  const [reason, setReason] = useState('');
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onCancel()}
      title={`Mover para ${column.name}`}
      description="Diga o que falta e quem pode destravar."
      footer={
        <>
          <Button variant="ghost" onClick={onCancel}>
            Cancelar
          </Button>
          <Button variant="primary" disabled={!reason.trim()} onClick={() => onConfirm(reason)}>
            Bloquear
          </Button>
        </>
      }
    >
      <Input
        label="Motivo"
        value={reason}
        autoFocus
        onChange={(e) => setReason(e.target.value)}
        placeholder="aguarda decisão do @arquiteto sobre legacy_id"
      />
    </Dialog>
  );
}
