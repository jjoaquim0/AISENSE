import type { Activity } from '@/types/generated/Activity';
import type { BoardView } from '@/types/generated/BoardView';
import type { CardView } from '@/types/generated/CardView';
import type { Column } from '@/types/generated/Column';

/** Quanto tempo o cartão que mudou fica realçado (docs/13, T8). */
export const HIGHLIGHT_MS = 400;

/** Filtros da tela: responsável (handle, `none` = sem dono) e label. */
export interface BoardFilter {
  assignee: string | null;
  label: string | null;
}

export function applyFilter(cards: CardView[], filter: BoardFilter): CardView[] {
  return cards.filter((c) => {
    if (filter.assignee === 'none' && c.assigneeHandle !== null) return false;
    if (filter.assignee && filter.assignee !== 'none' && c.assigneeHandle !== filter.assignee)
      return false;
    if (filter.label && !c.labels.includes(filter.label)) return false;
    return true;
  });
}

/** Cartões de cada coluna, na ordem do quadro. */
export function byColumn(view: BoardView, cards: CardView[] = view.cards): Map<string, CardView[]> {
  const map = new Map<string, CardView[]>(view.columns.map((c) => [c.id, []]));
  for (const card of cards) map.get(card.columnId)?.push(card);
  return map;
}

/** "2/4" com limite, "2" sem. */
export function countLabel(column: Column, count: number): string {
  return column.wipLimit === null ? String(count) : `${count}/${column.wipLimit}`;
}

/** Todas as labels em uso, para o filtro. */
export function labelsOf(cards: CardView[]): string[] {
  return [...new Set(cards.flatMap((c) => c.labels))].sort();
}

/**
 * Movimento otimista: o cartão aparece na coluna nova na hora. Se o core recusar (WIP,
 * motivo, gate), a tela volta para `view` — o estado anterior.
 */
export function moveLocally(view: BoardView, cardId: string, column: Column): BoardView {
  return {
    ...view,
    cards: view.cards.map((c) =>
      c.id === cardId ? { ...c, columnId: column.id, columnSlug: column.slug } : c,
    ),
  };
}

/** Quantos cartões mudaram desde que você saiu (um cartão conta uma vez). */
export function changedCards(activity: Activity[]): number {
  return new Set(activity.map((a) => a.cardId)).size;
}

const LAST_SEEN = 'aisense.board.seen.';

/** Última visita ao quadro desta equipe (conveniência deste navegador; falha vira "agora"). */
export function lastSeen(teamId: string, now: number): number {
  try {
    const raw = localStorage.getItem(LAST_SEEN + teamId);
    return raw ? Number(raw) : now;
  } catch {
    return now;
  }
}

export function markSeen(teamId: string, now: number): void {
  try {
    localStorage.setItem(LAST_SEEN + teamId, String(now));
  } catch {
    // Sem armazenamento: o contador só não lembra entre sessões.
  }
}

export function ago(now: number, then: number): string {
  const secs = Math.max(0, Math.floor((now - then) / 1000));
  if (secs < 60) return 'agora';
  if (secs < 3600) return `há ${Math.floor(secs / 60)}min`;
  if (secs < 86_400) return `há ${Math.floor(secs / 3600)}h`;
  return `há ${Math.floor(secs / 86_400)}d`;
}

/** `tsk_` + os 6 últimos — o mesmo id curto do `aisense board`. */
export function shortId(id: string): string {
  const ulid = id.startsWith('tsk_') ? id.slice(4) : id;
  return ulid.length > 6 ? `tsk_${ulid.slice(-6)}` : id;
}

export const PRIORITY_LABEL = {
  low: 'baixa',
  normal: 'média',
  high: 'alta',
  urgent: 'urgente',
} as const;
