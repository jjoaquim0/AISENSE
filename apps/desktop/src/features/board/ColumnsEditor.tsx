import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { Button, Dialog, IconButton } from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import type { BoardView } from '@/types/generated/BoardView';
import type { ColumnDraft } from '@/types/generated/ColumnDraft';
import type { ColumnKind } from '@/types/generated/ColumnKind';
import type { TeamId } from '@/types/generated/TeamId';
import { boardApi } from './api';
import { draftsOf, pendingMoves, slugify } from './columnsModel';

export const KIND_LABEL: Record<ColumnKind, string> = {
  intake: 'entrada',
  ready: 'pronto (puxar daqui)',
  active: 'em andamento',
  blocked: 'bloqueio (exige motivo)',
  review: 'revisão',
  terminal: 'concluído',
};

const cell = 'h-7 rounded-md border border-strong bg-surface px-1.5 text-label text-primary';

/**
 * Editor de colunas (F06-10): criar, renomear, reordenar, remover, `kind`, WIP e gate.
 * Remover coluna com cartões pede o destino deles — sem isso, "Salvar" não habilita.
 */
export function ColumnsEditor({
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
  const [drafts, setDrafts] = useState<ColumnDraft[]>(() => draftsOf(view.columns));
  const [targets, setTargets] = useState<Record<string, string>>({});
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const removed = pendingMoves(view, drafts);
  const missing = removed.some((r) => !targets[r.slug]);

  const edit = (i: number, patch: Partial<ColumnDraft>) =>
    setDrafts((list) => list.map((d, j) => (j === i ? { ...d, ...patch } : d)));
  const swap = (i: number, j: number) =>
    setDrafts((list) => {
      const next = [...list];
      const [a, b] = [next[i], next[j]];
      if (!a || !b) return list;
      next[i] = b;
      next[j] = a;
      return next;
    });
  const limit = (raw: string) => (raw.trim() === '' ? null : Math.max(1, Number(raw) || 1));

  const save = async () => {
    setBusy(true);
    setProblem(null);
    try {
      const moves = removed.map((r): [string, string] => [r.slug, targets[r.slug] ?? '']);
      await boardApi.saveColumns(teamId, drafts, moves);
      onSaved();
      onClose();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open
      size="lg"
      onOpenChange={(open) => !open && onClose()}
      title="Colunas do quadro"
      description="Todo quadro precisa de uma coluna pronta (de onde os agentes puxam) e uma de concluídos."
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button variant="primary" disabled={busy || missing} onClick={() => void save()}>
            Salvar colunas
          </Button>
        </>
      }
    >
      <table className="w-full text-label">
        <thead className="text-left text-caption text-muted">
          <tr>
            <th className="pb-1 font-medium">Nome</th>
            <th className="pb-1 font-medium">Slug</th>
            <th className="pb-1 font-medium">Tipo</th>
            <th className="pb-1 font-medium" title="Limite total de cartões">
              WIP
            </th>
            <th className="pb-1 font-medium" title="Limite por agente">
              Por agente
            </th>
            <th className="pb-1 font-medium" title="Só entra com aprovação (gate de revisão)">
              Aprovação
            </th>
            <th />
          </tr>
        </thead>
        <tbody>
          {drafts.map((d, i) => (
            <tr key={d.id ?? `nova-${i}`} className="align-middle">
              <td className="py-0.5 pr-1">
                <input
                  aria-label="Nome"
                  value={d.name}
                  className={`${cell} w-28`}
                  onChange={(e) =>
                    edit(
                      i,
                      d.id
                        ? { name: e.target.value }
                        : { name: e.target.value, slug: slugify(e.target.value) },
                    )
                  }
                />
              </td>
              <td className="py-0.5 pr-1">
                <input
                  aria-label="Slug"
                  value={d.slug}
                  className={`${cell} w-24 font-mono`}
                  onChange={(e) => edit(i, { slug: e.target.value })}
                />
              </td>
              <td className="py-0.5 pr-1">
                <select
                  aria-label="Tipo"
                  value={d.kind}
                  className={cell}
                  onChange={(e) => edit(i, { kind: e.target.value as ColumnKind })}
                >
                  {(Object.keys(KIND_LABEL) as ColumnKind[]).map((k) => (
                    <option key={k} value={k}>
                      {KIND_LABEL[k]}
                    </option>
                  ))}
                </select>
              </td>
              <td className="py-0.5 pr-1">
                <input
                  aria-label="Limite de WIP"
                  inputMode="numeric"
                  value={d.wipLimit ?? ''}
                  placeholder="—"
                  className={`${cell} w-12`}
                  onChange={(e) => edit(i, { wipLimit: limit(e.target.value) })}
                />
              </td>
              <td className="py-0.5 pr-1">
                <input
                  aria-label="Limite por agente"
                  inputMode="numeric"
                  value={d.wipPerAgent ?? ''}
                  placeholder="—"
                  className={`${cell} w-12`}
                  onChange={(e) => edit(i, { wipPerAgent: limit(e.target.value) })}
                />
              </td>
              <td className="py-0.5 pr-1 text-center">
                <input
                  type="checkbox"
                  aria-label="Exige aprovação"
                  checked={d.requiresApproval}
                  onChange={(e) => edit(i, { requiresApproval: e.target.checked })}
                />
              </td>
              <td className="flex gap-0.5 py-0.5">
                <IconButton
                  label="Subir"
                  size="sm"
                  disabled={i === 0}
                  onClick={() => swap(i, i - 1)}
                >
                  <ArrowUp size={12} />
                </IconButton>
                <IconButton
                  label="Descer"
                  size="sm"
                  disabled={i === drafts.length - 1}
                  onClick={() => swap(i, i + 1)}
                >
                  <ArrowDown size={12} />
                </IconButton>
                <IconButton
                  label={`Remover ${d.name}`}
                  size="sm"
                  onClick={() => setDrafts((list) => list.filter((_, j) => j !== i))}
                >
                  <Trash2 size={12} />
                </IconButton>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <Button
        size="sm"
        variant="ghost"
        className="mt-2"
        onClick={() =>
          setDrafts((list) => [
            ...list,
            {
              id: null,
              slug: '',
              name: '',
              kind: 'active',
              wipLimit: null,
              wipPerAgent: null,
              requiresApproval: false,
              approverMustDiffer: true,
              requiresCommands: [],
            },
          ])
        }
      >
        <Plus size={12} /> Coluna
      </Button>

      {removed.length > 0 && (
        <section className="mt-3 flex flex-col gap-1.5 rounded-md border border-subtle p-2">
          {removed.map((r) => (
            <label key={r.slug} className="flex items-center gap-2 text-label text-secondary">
              {r.name} tem {r.count} {r.count === 1 ? 'cartão' : 'cartões'}: mover para
              <select
                aria-label={`Destino dos cartões de ${r.name}`}
                value={targets[r.slug] ?? ''}
                className={cell}
                onChange={(e) => setTargets((t) => ({ ...t, [r.slug]: e.target.value }))}
              >
                <option value="">escolha…</option>
                {drafts
                  .filter((d) => d.slug)
                  .map((d) => (
                    <option key={d.slug} value={d.slug}>
                      {d.name || d.slug}
                    </option>
                  ))}
              </select>
            </label>
          ))}
        </section>
      )}
      {problem && (
        <p role="alert" className="mt-2 text-caption text-failed">
          {problem}
        </p>
      )}
    </Dialog>
  );
}
