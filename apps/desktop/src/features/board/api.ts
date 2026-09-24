import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Activity } from '@/types/generated/Activity';
import type { Automation } from '@/types/generated/Automation';
import type { Board } from '@/types/generated/Board';
import type { BoardEvent } from '@/types/generated/BoardEvent';
import type { BoardView } from '@/types/generated/BoardView';
import type { CardDetail } from '@/types/generated/CardDetail';
import type { CardPatch } from '@/types/generated/CardPatch';
import type { Column } from '@/types/generated/Column';
import type { ColumnDraft } from '@/types/generated/ColumnDraft';
import type { Comment } from '@/types/generated/Comment';
import type { LinkKind } from '@/types/generated/LinkKind';
import type { Moved } from '@/types/generated/Moved';
import type { NewCard } from '@/types/generated/NewCard';
import type { TeamId } from '@/types/generated/TeamId';

/**
 * Único ponto do front que chama os comandos do quadro (docs/13). As regras (WIP, motivo,
 * gate, automações) são do core: a UI mostra o erro que ele devolver, igual ao da CLI.
 */
export const boardApi = {
  get: (teamId: TeamId): Promise<BoardView> => invoke('board_get', { teamId }),
  changes: (teamId: TeamId, since: number): Promise<Activity[]> =>
    invoke('board_changes', { teamId, since }),
  show: (teamId: TeamId, id: string): Promise<CardDetail> => invoke('card_show', { teamId, id }),
  add: (teamId: TeamId, card: NewCard): Promise<Moved> => invoke('card_add', { teamId, card }),
  move: (teamId: TeamId, id: string, column: string, reason?: string): Promise<Moved> =>
    invoke('card_move', { teamId, id, column, reason: reason ?? null }),
  update: (teamId: TeamId, id: string, patch: Partial<CardPatch>): Promise<Moved> =>
    invoke('card_update', { teamId, id, patch }),
  check: (teamId: TeamId, id: string, item: number, done: boolean): Promise<Moved> =>
    invoke('card_check', { teamId, id, item, done }),
  comment: (teamId: TeamId, id: string, body: string): Promise<Comment> =>
    invoke('card_comment', { teamId, id, body }),
  link: (teamId: TeamId, id: string, kind: LinkKind, target: string): Promise<Moved> =>
    invoke('card_link', { teamId, id, kind, target }),
  approve: (teamId: TeamId, id: string, note?: string): Promise<Moved> =>
    invoke('card_approve', { teamId, id, note: note ?? null }),
  reject: (teamId: TeamId, id: string, reason: string): Promise<Moved> =>
    invoke('card_reject', { teamId, id, reason }),
  archive: (teamId: TeamId, id: string): Promise<Moved> => invoke('card_archive', { teamId, id }),
  saveColumns: (
    teamId: TeamId,
    columns: ColumnDraft[],
    moves: [string, string][],
  ): Promise<Column[]> => invoke('board_columns_save', { teamId, columns, moves }),
  saveAutomations: (teamId: TeamId, automations: Automation[]): Promise<Board> =>
    invoke('board_automations_save', { teamId, automations }),
  automationsToml: (automations: Automation[]): Promise<string> =>
    invoke('board_automations_toml', { automations }),
  parseAutomations: (source: string): Promise<Automation[]> =>
    invoke('board_automations_parse', { source }),
};

/** Toda mudança do quadro, de qualquer equipe (UI, CLI, MCP ou automação). */
export function onBoardChanged(handler: (event: BoardEvent) => void): Promise<UnlistenFn> {
  return listen<BoardEvent>('board:changed', ({ payload }) => handler(payload));
}
