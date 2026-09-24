import type { Action } from '@/types/generated/Action';
import type { Trigger } from '@/types/generated/Trigger';

export const TRIGGER_LABEL: Record<Trigger, string> = {
  card_created: 'cartão criado',
  card_enters: 'cartão entra em',
  card_leaves: 'cartão sai de',
  card_stale: 'cartão parado em',
  checklist_complete: 'checklist completo',
  comment_added: 'comentário novo',
};

export type ActionKind =
  | 'assign'
  | 'notify'
  | 'move'
  | 'add_label'
  | 'unblock_dependents'
  | 'create_card';

export const ACTION_KINDS: { kind: ActionKind; label: string }[] = [
  { kind: 'assign', label: 'atribuir a' },
  { kind: 'notify', label: 'avisar' },
  { kind: 'move', label: 'mover para' },
  { kind: 'add_label', label: 'pôr label' },
  { kind: 'unblock_dependents', label: 'liberar dependentes' },
  { kind: 'create_card', label: 'criar cartão' },
];

/** A variante é a chave do objeto (o TOML de docs/13), não uma tag. */
export function actionKind(action: Action): ActionKind {
  if ('assign' in action) return 'assign';
  if ('notify' in action) return 'notify';
  if ('move' in action) return 'move';
  if ('add_label' in action) return 'add_label';
  if ('create_card' in action) return 'create_card';
  return 'unblock_dependents';
}

export function blankAction(kind: ActionKind): Action {
  switch (kind) {
    case 'assign':
      return { assign: 'actor' };
    case 'notify':
      return { notify: 'assignee', message: '' };
    case 'move':
      return { move: '' };
    case 'add_label':
      return { add_label: '' };
    case 'unblock_dependents':
      return { unblock_dependents: true };
    case 'create_card':
      return { create_card: '', column: null };
  }
}
