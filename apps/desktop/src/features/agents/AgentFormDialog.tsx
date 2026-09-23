import { Dialog } from '@/components/ui';
import type { Agent } from '@/types/generated/Agent';
import { AgentFormFields, type AgentFormFieldsProps } from './AgentFormFields';

interface AgentFormDialogProps
  extends Omit<AgentFormFieldsProps, 'active' | 'onCancel' | 'compact'> {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** T5 — Criar/editar agente (docs/09) num diálogo. Os campos estão em `AgentFormFields`. */
export function AgentFormDialog({ open, onOpenChange, onSaved, ...form }: AgentFormDialogProps) {
  const editing = form.agent !== undefined;
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="lg"
      title={editing ? `Editar @${form.agent?.handle}` : 'Novo agente'}
      description={
        editing
          ? 'As mudanças são gravadas na hora.'
          : 'Um agente é um terminal com um runtime de IA e um papel na equipe.'
      }
    >
      <AgentFormFields
        {...form}
        active={open}
        onCancel={() => onOpenChange(false)}
        onSaved={(agent: Agent, restartRequired: boolean) => {
          onSaved(agent, restartRequired);
          onOpenChange(false);
        }}
      />
    </Dialog>
  );
}
