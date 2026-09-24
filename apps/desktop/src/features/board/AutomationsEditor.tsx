import { Plus, Trash2 } from 'lucide-react';
import { type ReactNode, useEffect, useState } from 'react';
import { Button, Dialog, IconButton } from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import type { Action } from '@/types/generated/Action';
import type { Automation } from '@/types/generated/Automation';
import type { BoardView } from '@/types/generated/BoardView';
import type { TeamId } from '@/types/generated/TeamId';
import type { Trigger } from '@/types/generated/Trigger';
import { boardApi } from './api';
import {
  ACTION_KINDS,
  type ActionKind,
  actionKind,
  blankAction,
  TRIGGER_LABEL,
} from './automationModel';

const cell = 'h-7 rounded-md border border-strong bg-surface px-1.5 text-label text-primary';

/**
 * Editor de automações (F06-10): formulário com os gatilhos e ações do conjunto fechado,
 * com o TOML gerado à vista. Quem valida é o core — o erro dele aparece aqui.
 */
export function AutomationsEditor({
  teamId,
  view,
  onClose,
  onSaved,
}: {
  teamId: TeamId;
  view: BoardView;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [rules, setRules] = useState<Automation[]>(view.board.automations);
  const [toml, setToml] = useState('');
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    boardApi
      .automationsToml(rules)
      .then((text) => alive && setToml(text))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [rules]);

  const edit = (i: number, patch: Partial<Automation>) =>
    setRules((list) => list.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const editAction = (i: number, k: number, action: Action) =>
    setRules((list) =>
      list.map((r, j) =>
        j === i ? { ...r, then: r.then.map((a, m) => (m === k ? action : a)) } : r,
      ),
    );

  const save = async () => {
    setBusy(true);
    setProblem(null);
    try {
      await boardApi.saveAutomations(teamId, rules);
      onSaved();
      onClose();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const columnSelect = (value: string | null | undefined, onChange: (v: string | null) => void) => (
    <select
      aria-label="Coluna"
      value={value ?? ''}
      className={cell}
      onChange={(e) => onChange(e.target.value || null)}
    >
      <option value="">qualquer coluna</option>
      {view.columns.map((c) => (
        <option key={c.id} value={c.slug}>
          {c.name}
        </option>
      ))}
    </select>
  );

  return (
    <Dialog
      open
      size="lg"
      onOpenChange={(open) => !open && onClose()}
      title="Automações do quadro"
      description="Gatilhos e ações de um conjunto fechado: nenhuma automação roda comando."
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button variant="primary" disabled={busy} onClick={() => void save()}>
            Salvar automações
          </Button>
        </>
      }
    >
      <ol className="flex flex-col gap-2">
        {rules.map((rule, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: a automação não tem id; a ordem é a identidade
          <li key={i} className="flex flex-col gap-1.5 rounded-md border border-subtle p-2">
            <div className="flex flex-wrap items-center gap-1.5 text-label text-secondary">
              Quando
              <select
                aria-label="Gatilho"
                value={rule.when}
                className={cell}
                onChange={(e) => edit(i, { when: e.target.value as Trigger })}
              >
                {(Object.keys(TRIGGER_LABEL) as Trigger[]).map((t) => (
                  <option key={t} value={t}>
                    {TRIGGER_LABEL[t]}
                  </option>
                ))}
              </select>
              {columnSelect(rule.column, (column) => edit(i, { column }))}
              {rule.when === 'card_stale' && (
                <label className="flex items-center gap-1">
                  depois de
                  <input
                    aria-label="Horas"
                    inputMode="decimal"
                    value={rule.after_h ?? ''}
                    className={`${cell} w-14`}
                    onChange={(e) => edit(i, { after_h: Number(e.target.value) || null })}
                  />
                  h
                </label>
              )}
              <IconButton
                label="Remover automação"
                size="sm"
                className="ml-auto"
                onClick={() => setRules((list) => list.filter((_, j) => j !== i))}
              >
                <Trash2 size={12} />
              </IconButton>
            </div>
            {rule.then.map((action, k) => (
              <ActionRow
                // biome-ignore lint/suspicious/noArrayIndexKey: ações não têm id
                key={k}
                action={action}
                columnSelect={columnSelect}
                onChange={(a) => editAction(i, k, a)}
                onRemove={() => edit(i, { then: rule.then.filter((_, m) => m !== k) })}
              />
            ))}
            <Button
              size="sm"
              variant="ghost"
              className="self-start"
              onClick={() => edit(i, { then: [...rule.then, blankAction('notify')] })}
            >
              <Plus size={12} /> Ação
            </Button>
          </li>
        ))}
      </ol>
      <Button
        size="sm"
        variant="ghost"
        className="mt-2"
        onClick={() =>
          setRules((list) => [
            ...list,
            {
              when: 'card_enters',
              column: view.columns[0]?.slug ?? null,
              then: [blankAction('notify')],
            },
          ])
        }
      >
        <Plus size={12} /> Automação
      </Button>
      {problem && (
        <p role="alert" className="mt-2 text-caption text-failed">
          {problem}
        </p>
      )}
      <details className="mt-3">
        <summary className="cursor-default text-label text-secondary">TOML gerado</summary>
        <pre className="mt-1 max-h-48 overflow-auto rounded-md bg-terminal p-2 font-mono text-caption text-primary select-text">
          {toml}
        </pre>
      </details>
    </Dialog>
  );
}

function ActionRow({
  action,
  columnSelect,
  onChange,
  onRemove,
}: {
  action: Action;
  columnSelect: (
    value: string | null | undefined,
    onChange: (v: string | null) => void,
  ) => ReactNode;
  onChange: (action: Action) => void;
  onRemove: () => void;
}) {
  const kind = actionKind(action);
  const text = (label: string, value: string, set: (v: string) => void, width = 'w-32') => (
    <input
      aria-label={label}
      placeholder={label}
      value={value}
      className={`${cell} ${width}`}
      onChange={(e) => set(e.target.value)}
    />
  );
  return (
    <div className="flex flex-wrap items-center gap-1.5 pl-4 text-label text-secondary">
      então
      <select
        aria-label="Ação"
        value={kind}
        className={cell}
        onChange={(e) => onChange(blankAction(e.target.value as ActionKind))}
      >
        {ACTION_KINDS.map((k) => (
          <option key={k.kind} value={k.kind}>
            {k.label}
          </option>
        ))}
      </select>
      {'assign' in action &&
        text('@handle ou actor', action.assign, (assign) => onChange({ assign }))}
      {'notify' in action && (
        <>
          {text('@handle, assignee ou creator', action.notify, (notify) =>
            onChange({ ...action, notify }),
          )}
          {text('mensagem', action.message, (message) => onChange({ ...action, message }), 'w-56')}
        </>
      )}
      {'move' in action && columnSelect(action.move, (move) => onChange({ move: move ?? '' }))}
      {'add_label' in action &&
        text('label', action.add_label, (add_label) => onChange({ add_label }))}
      {'create_card' in action && (
        <>
          {text('título do cartão', action.create_card, (create_card) =>
            onChange({ ...action, create_card }),
          )}
          {columnSelect(action.column, (column) => onChange({ ...action, column }))}
        </>
      )}
      <IconButton label="Remover ação" size="sm" onClick={onRemove}>
        <Trash2 size={12} />
      </IconButton>
    </div>
  );
}
