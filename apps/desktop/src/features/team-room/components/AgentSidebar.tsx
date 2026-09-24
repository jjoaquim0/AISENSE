import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { GripVertical, Pencil, Play, Plus, Square, Trash2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { formatShortcut, IconButton, ScrollArea, StatusDot, Tooltip } from '@/components/ui';
import { cn } from '@/lib/cn';
import type { Agent } from '@/types/generated/Agent';
import type { AgentColor } from '@/types/generated/AgentColor';
import type { AgentState } from '@/types/generated/AgentState';
import type { StateConfidence } from '@/types/generated/StateConfidence';
import { reorder } from '../gridLayout';
import { announcement, isRunning } from '../sidebar';

/** Mensagens na caixa do agente, na cor de quem mandou (docs/08). Chega na Fase 05. */
export interface PendingBadge {
  count: number;
  color: AgentColor;
}

interface AgentSidebarProps {
  /** Agentes na ordem da equipe (`position`). */
  agents: Agent[];
  selectedId: string | null;
  stateOf: (id: string) => AgentState;
  confidenceOf: (id: string) => StateConfidence | undefined;
  pendingOf?: (id: string) => PendingBadge | undefined;
  /** Clique: foca o painel do agente. */
  onSelect: (id: string) => void;
  /** Duplo clique: abre o inspetor. */
  onInspect: (id: string) => void;
  /** Ordem nova da equipe inteira, depois de arrastar. */
  onReorder: (ids: string[]) => void;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onEdit: (agent: Agent) => void;
  onDelete: (agent: Agent) => void;
  onAdd: () => void;
}

/**
 * Sidebar de agentes da Sala da Equipe (docs/09, "Estrutura global da janela"): estado
 * ao vivo, badge de mensagens, arrastar para reordenar. A ordem é a da equipe — a mesma
 * em que ▶ sobe os agentes —, não a dos painéis da Grid.
 */
export function AgentSidebar(props: AgentSidebarProps) {
  const { agents, onReorder, onAdd } = props;
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const ids = agents.map((a) => a.id);

  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (over && active.id !== over.id) onReorder(reorder(ids, String(active.id), String(over.id)));
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <ScrollArea className="flex-1">
        <section className="px-2 py-3">
          <h2 className="px-2.5 pb-1 text-caption tracking-[0.02em] text-muted uppercase">
            Agentes
          </h2>
          {agents.length === 0 && (
            <p className="px-2.5 py-1.5 text-caption text-muted">
              Nenhum agente ainda. Crie um com {formatShortcut('⌘T')}.
            </p>
          )}
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={onDragEnd}
            accessibility={{ screenReaderInstructions: { draggable: INSTRUCTIONS } }}
          >
            <SortableContext items={ids} strategy={verticalListSortingStrategy}>
              <ul aria-label="Agentes" className="flex flex-col gap-0.5">
                {agents.map((agent) => (
                  <SidebarRow key={agent.id} agent={agent} {...props} />
                ))}
              </ul>
            </SortableContext>
          </DndContext>
          <button
            type="button"
            onClick={onAdd}
            className="mt-1 flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-caption text-muted transition-colors duration-100 hover:bg-hover hover:text-primary"
          >
            <Plus size={13} /> Novo agente
          </button>
        </section>
      </ScrollArea>
      <StateAnnouncer agents={agents} stateOf={props.stateOf} />
    </div>
  );
}

const INSTRUCTIONS =
  'Para reordenar, pressione espaço ou Enter, use as setas para mover e espaço de novo para soltar. Esc cancela.';

function SidebarRow({
  agent,
  selectedId,
  stateOf,
  confidenceOf,
  pendingOf,
  onSelect,
  onInspect,
  onStart,
  onStop,
  onEdit,
  onDelete,
}: AgentSidebarProps & { agent: Agent }) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: agent.id });
  const state = stateOf(agent.id);
  const running = isRunning(state);
  const selected = agent.id === selectedId;
  const pending = pendingOf?.(agent.id);

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={cn('relative', isDragging && 'z-10 opacity-80')}
    >
      <div
        className={cn(
          'group relative flex items-center gap-1 rounded-lg py-1 pr-1 pl-2.5',
          selected ? 'bg-active' : 'hover:bg-hover',
          isDragging && 'bg-raised shadow-md',
        )}
      >
        <span
          aria-hidden
          className="absolute inset-y-1 left-0 w-[3px] rounded-full"
          style={{ background: `var(--agent-${agent.color})` }}
        />
        <button
          type="button"
          aria-current={selected || undefined}
          onClick={() => onSelect(agent.id)}
          onDoubleClick={() => onInspect(agent.id)}
          title="Clique para focar · duplo clique abre o inspetor"
          className="flex min-w-0 flex-1 items-center gap-2 py-0.5 text-left"
        >
          <span className="flex min-w-0 flex-col">
            <span className="truncate text-body text-primary">@{agent.handle}</span>
            <span className="flex min-w-0 items-center gap-1.5">
              <span className="relative flex shrink-0">
                <StatusDot state={state} confidence={confidenceOf(agent.id)} withLabel />
                {pending && pending.count > 0 && <PendingCount pending={pending} />}
              </span>
              <span className="truncate text-caption text-muted">· {agent.adapterId}</span>
            </span>
          </span>
        </button>
        {/* Só aparecem com o ponteiro ou o foco na linha: a 240px o handle precisa do espaço. */}
        <div className="hidden items-center group-focus-within:flex group-hover:flex">
          {running ? (
            <Tooltip content="Parar">
              <IconButton
                label={`Parar @${agent.handle}`}
                size="sm"
                onClick={() => onStop(agent.id)}
              >
                <Square size={12} />
              </IconButton>
            </Tooltip>
          ) : (
            <Tooltip content="Iniciar">
              <IconButton
                label={`Iniciar @${agent.handle}`}
                size="sm"
                onClick={() => onStart(agent.id)}
              >
                <Play size={12} />
              </IconButton>
            </Tooltip>
          )}
          <Tooltip content="Editar">
            <IconButton label={`Editar @${agent.handle}`} size="sm" onClick={() => onEdit(agent)}>
              <Pencil size={12} />
            </IconButton>
          </Tooltip>
          <Tooltip content="Excluir">
            <IconButton
              label={`Excluir @${agent.handle}`}
              size="sm"
              variant="danger"
              onClick={() => onDelete(agent)}
            >
              <Trash2 size={12} />
            </IconButton>
          </Tooltip>
          <button
            ref={setActivatorNodeRef}
            type="button"
            {...attributes}
            {...listeners}
            aria-label={`Reordenar @${agent.handle}`}
            aria-roledescription="item arrastável"
            className="flex size-6 cursor-grab items-center justify-center rounded-md text-muted hover:bg-hover hover:text-primary active:cursor-grabbing"
          >
            <GripVertical size={12} />
          </button>
        </div>
      </div>
    </li>
  );
}

/** Sobre o ponto de estado, na cor do remetente (docs/08). */
function PendingCount({ pending }: { pending: PendingBadge }) {
  const { count } = pending;
  return (
    <span
      role="img"
      aria-label={`${count} ${count === 1 ? 'mensagem pendente' : 'mensagens pendentes'}`}
      className="absolute -top-2 left-1 min-w-3.5 rounded-full px-1 text-center text-[10px] leading-3.5 font-medium text-accent-fg"
      style={{ background: `var(--agent-${pending.color})` }}
    >
      {count > 9 ? '9+' : count}
    </span>
  );
}

/**
 * Anuncia a leitores de tela as mudanças que pedem atenção (docs/08, acessibilidade).
 * Trabalhando/ocioso alternam a cada poucos segundos e virariam ruído constante.
 */
function StateAnnouncer({
  agents,
  stateOf,
}: {
  agents: Agent[];
  stateOf: (id: string) => AgentState;
}) {
  const previous = useRef<Record<string, AgentState>>({});
  const [message, setMessage] = useState('');
  const current = agents.map((a) => `${a.id}:${stateOf(a.id)}`).join('|');

  // biome-ignore lint/correctness/useExhaustiveDependencies: `current` resume os estados.
  useEffect(() => {
    const said: string[] = [];
    for (const agent of agents) {
      const state = stateOf(agent.id);
      const before = previous.current[agent.id];
      if (before !== undefined && before !== state) {
        const text = announcement(agent.handle, state);
        if (text) said.push(text);
      }
      previous.current[agent.id] = state;
    }
    if (said.length > 0) setMessage(said.join('. '));
  }, [current]);

  return (
    <p role="status" aria-live="polite" className="sr-only">
      {message}
    </p>
  );
}
