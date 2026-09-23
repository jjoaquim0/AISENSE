import { useState } from 'react';
import { Button, Dialog, Input } from '@/components/ui';
import type { Team } from '@/types/generated/Team';
import { errorMessage, teamsApi } from './api';

interface DeleteTeamDialogProps {
  team: Team;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeleted: () => void;
}

/**
 * Excluir leva junto agentes, mensagens, tarefas e sessões (I5). Por isso exige digitar o
 * nome da equipe (docs/09, F02-08) — e o core confere de novo.
 */
export function DeleteTeamDialog({ team, open, onOpenChange, onDeleted }: DeleteTeamDialogProps) {
  const [typed, setTyped] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const matches = typed.trim() === team.name;

  const close = (next: boolean) => {
    if (!next) {
      setTyped('');
      setError(null);
    }
    onOpenChange(next);
  };

  const remove = async () => {
    setBusy(true);
    try {
      await teamsApi.remove(team.id, typed);
      close(false);
      onDeleted();
    } catch (e: unknown) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={close}
      title={`Excluir “${team.name}”?`}
      description="Os agentes são parados e tudo o que é da equipe é apagado: agentes, mensagens, tarefas e histórico. Não dá para desfazer."
      footer={
        <>
          <Button variant="ghost" onClick={() => close(false)}>
            Cancelar
          </Button>
          <Button variant="danger" disabled={!matches || busy} onClick={() => void remove()}>
            Excluir equipe
          </Button>
        </>
      }
    >
      <Input
        label={`Digite ${team.name} para confirmar`}
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        autoComplete="off"
        error={error ?? undefined}
        autoFocus
      />
    </Dialog>
  );
}
