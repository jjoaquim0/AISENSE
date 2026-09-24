import type { BoardView } from '@/types/generated/BoardView';
import type { Column } from '@/types/generated/Column';
import type { ColumnDraft } from '@/types/generated/ColumnDraft';

export function draftsOf(columns: Column[]): ColumnDraft[] {
  return columns.map((c) => ({
    id: c.id,
    slug: c.slug,
    name: c.name,
    kind: c.kind,
    wipLimit: c.wipLimit,
    wipPerAgent: c.wipPerAgent,
    requiresApproval: c.requiresApproval,
    approverMustDiffer: c.approverMustDiffer,
    requiresCommands: c.requiresCommands,
  }));
}

/** "Em Revisão" → "em-revisao": o slug que a CLI usa. */
export function slugify(name: string): string {
  return name
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 32);
}

/** Colunas que saem do quadro e ainda têm cartões: precisam de destino. */
export function pendingMoves(
  view: BoardView,
  drafts: ColumnDraft[],
): { slug: string; name: string; count: number }[] {
  const kept = new Set(drafts.map((d) => d.id).filter(Boolean));
  return view.columns
    .filter((c) => !kept.has(c.id))
    .map((c) => ({
      slug: c.slug,
      name: c.name,
      count: view.cards.filter((card) => card.columnId === c.id).length,
    }))
    .filter((r) => r.count > 0);
}
