import { useEffect, useState } from 'react';
import { Button, Dialog, Input } from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import type { AgentTag } from '@/types/generated/AgentTag';
import type { CardPriority } from '@/types/generated/CardPriority';
import type { Column } from '@/types/generated/Column';
import type { TeamId } from '@/types/generated/TeamId';
import { boardApi } from './api';
import { PRIORITY_LABEL } from './boardModel';

const selectClass = 'h-8 rounded-md border border-strong bg-surface px-1.5 text-body text-primary';

/** `+ Cartão`: o mesmo `aisense task add`, com os campos do doc. */
export function NewCardDialog({
  open,
  teamId,
  columns,
  agents,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  teamId: TeamId;
  columns: Column[];
  agents: AgentTag[];
  onOpenChange: (open: boolean) => void;
  onCreated: () => void;
}) {
  const ready = columns.find((c) => c.kind === 'ready')?.slug ?? columns[0]?.slug ?? '';
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [column, setColumn] = useState(ready);
  const [assignee, setAssignee] = useState('');
  const [priority, setPriority] = useState<CardPriority>('normal');
  const [labels, setLabels] = useState('');
  const [checklist, setChecklist] = useState('');
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setTitle('');
      setBody('');
      setColumn(ready);
      setAssignee('');
      setPriority('normal');
      setLabels('');
      setChecklist('');
      setProblem(null);
    }
  }, [open, ready]);

  const create = async () => {
    setBusy(true);
    setProblem(null);
    try {
      await boardApi.add(teamId, {
        title,
        body,
        column,
        assignee: assignee || null,
        labels: labels ? [labels] : [],
        priority,
        blockedBy: [],
        checklist: checklist ? [checklist] : [],
        parent: null,
        reason: null,
      });
      onCreated();
      onOpenChange(false);
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Novo cartão"
      description="Os agentes veem o cartão na hora, pela CLI e pelo MCP."
      footer={
        <>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancelar
          </Button>
          <Button variant="primary" disabled={busy || !title.trim()} onClick={() => void create()}>
            Criar cartão
          </Button>
        </>
      }
    >
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          void create();
        }}
      >
        <Input label="Título" value={title} autoFocus onChange={(e) => setTitle(e.target.value)} />
        <label className="flex flex-col gap-1 text-label text-secondary">
          Descrição (Markdown)
          <textarea
            value={body}
            rows={4}
            onChange={(e) => setBody(e.target.value)}
            className="rounded-md border border-strong bg-surface px-2.5 py-1.5 text-body text-primary"
          />
        </label>
        <div className="grid grid-cols-3 gap-2">
          <label className="flex flex-col gap-1 text-label text-secondary">
            Coluna
            <select
              value={column}
              onChange={(e) => setColumn(e.target.value)}
              className={selectClass}
            >
              {columns
                .filter((c) => !c.requiresApproval)
                .map((c) => (
                  <option key={c.id} value={c.slug}>
                    {c.name}
                  </option>
                ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-label text-secondary">
            Responsável
            <select
              value={assignee}
              onChange={(e) => setAssignee(e.target.value)}
              className={selectClass}
            >
              <option value="">ninguém</option>
              {agents.map((a) => (
                <option key={a.id} value={a.handle}>
                  @{a.handle}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-label text-secondary">
            Prioridade
            <select
              value={priority}
              onChange={(e) => setPriority(e.target.value as CardPriority)}
              className={selectClass}
            >
              {(Object.keys(PRIORITY_LABEL) as CardPriority[]).map((p) => (
                <option key={p} value={p}>
                  {PRIORITY_LABEL[p]}
                </option>
              ))}
            </select>
          </label>
        </div>
        <Input
          label="Labels"
          hint="Separadas por vírgula"
          value={labels}
          onChange={(e) => setLabels(e.target.value)}
        />
        <Input
          label="Checklist"
          hint="Itens separados por vírgula"
          value={checklist}
          onChange={(e) => setChecklist(e.target.value)}
        />
        {problem && (
          <p role="alert" className="text-caption text-failed">
            {problem}
          </p>
        )}
      </form>
    </Dialog>
  );
}
