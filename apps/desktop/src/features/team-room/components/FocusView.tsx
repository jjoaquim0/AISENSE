import { Plus } from 'lucide-react';
import type { ReactNode } from 'react';
import type { Agent } from '@/types/generated/Agent';
import type { AgentState } from '@/types/generated/AgentState';
import type { StateConfidence } from '@/types/generated/StateConfidence';
import { usePreviews } from '../hooks/usePreviews';
import { focusTarget } from '../roomView';
import { MiniPreview } from './MiniPreview';

interface FocusViewProps {
  /** Ordem dos agentes (a mesma da Grid). */
  order: string[];
  agents: Agent[];
  focusedId: string | null;
  stateOf: (id: string) => AgentState;
  confidenceOf: (id: string) => StateConfidence | undefined;
  renderPane: (id: string) => ReactNode;
  onFocus: (id: string) => void;
  onAdd: () => void;
}

/**
 * Vista Foco (docs/09, T4.2): um terminal grande e uma tira de miniaturas leves dos
 * outros agentes. Só o painel grande tem xterm.
 */
export function FocusView({
  order,
  agents,
  focusedId,
  stateOf,
  confidenceOf,
  renderPane,
  onFocus,
  onAdd,
}: FocusViewProps) {
  const target = focusTarget(order, focusedId);
  const others = order.filter((id) => id !== target);
  const previews = usePreviews(others);
  const byId = new Map(agents.map((a) => [a.id, a]));

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex min-h-0 flex-1">{target && renderPane(target)}</div>
      <nav aria-label="Outros agentes" className="flex h-24 shrink-0 gap-2 overflow-x-auto">
        {others.map((id) => {
          const agent = byId.get(id);
          if (!agent) return null;
          return (
            <MiniPreview
              key={id}
              agent={agent}
              state={stateOf(id)}
              confidence={confidenceOf(id)}
              lines={previews[id] ?? []}
              index={order.indexOf(id)}
              onSelect={() => onFocus(id)}
            />
          );
        })}
        <button
          type="button"
          onClick={onAdd}
          aria-label="Novo agente"
          className="flex h-full w-16 shrink-0 items-center justify-center rounded-lg border border-subtle border-dashed text-muted hover:bg-hover hover:text-primary"
        >
          <Plus size={16} />
        </button>
      </nav>
    </div>
  );
}
