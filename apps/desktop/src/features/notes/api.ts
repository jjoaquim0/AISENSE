import { invoke } from '@tauri-apps/api/core';
import type { Note } from '@/types/generated/Note';
import type { NoteMatch } from '@/types/generated/NoteMatch';
import type { NoteSave } from '@/types/generated/NoteSave';
import type { NoteSummary } from '@/types/generated/NoteSummary';
import type { TeamId } from '@/types/generated/TeamId';

/** Único ponto do front que chama os comandos de notas da equipe (docs/15). */
export const notesApi = {
  /** Mais recente primeiro. */
  list: (teamId: TeamId): Promise<NoteSummary[]> => invoke('notes_list', { teamId }),
  read: (teamId: TeamId, slug: string): Promise<Note> => invoke('note_read', { teamId, slug }),
  create: (teamId: TeamId, slug: string, title: string): Promise<Note> =>
    invoke('note_create', { teamId, slug, title }),
  /** Trava otimista: com a nota mudada desde a leitura volta `stale`, com o diff. */
  save: (
    teamId: TeamId,
    slug: string,
    content: string,
    expectHash: string | null,
  ): Promise<NoteSave> => invoke('note_save', { teamId, slug, content, expectHash }),
  remove: (teamId: TeamId, slug: string): Promise<void> => invoke('note_delete', { teamId, slug }),
  search: (teamId: TeamId, query: string): Promise<NoteMatch[]> =>
    invoke('notes_search', { teamId, query }),
};
