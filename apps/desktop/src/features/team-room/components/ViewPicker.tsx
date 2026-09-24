import { KanbanSquare, LayoutGrid, MessagesSquare, SquareSplitVertical } from 'lucide-react';
import { formatShortcut } from '@/components/ui';
import { cn } from '@/lib/cn';
import { ROOM_VIEWS, type RoomView } from '../roomView';

const VIEWS: Record<RoomView, { label: string; icon: typeof LayoutGrid }> = {
  grid: { label: 'Grade', icon: LayoutGrid },
  focus: { label: 'Foco', icon: SquareSplitVertical },
  timeline: { label: 'Mensagens', icon: MessagesSquare },
  board: { label: 'Quadro', icon: KanbanSquare },
};

/** Seletor de vista da Sala da Equipe (Grade · Foco · Linha do tempo · Quadro; Fluxo vem na fase 7). */
export function ViewPicker({
  value,
  onChange,
}: {
  value: RoomView;
  onChange: (view: RoomView) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label="Vista"
      className="flex items-center rounded-md border border-subtle p-0.5"
    >
      {ROOM_VIEWS.map((view) => {
        const { label, icon: Icon } = VIEWS[view];
        return (
          // biome-ignore lint/a11y/useSemanticElements: botões segmentados com papel de rádio, padrão WAI-ARIA
          <button
            key={view}
            type="button"
            role="radio"
            aria-checked={value === view}
            onClick={() => onChange(view)}
            title={`${view === 'timeline' ? 'Linha do tempo' : label} · ${formatShortcut('⌘G')} alterna a vista`}
            className={cn(
              'flex items-center gap-1 rounded px-2 py-0.5 text-caption whitespace-nowrap',
              value === view ? 'bg-active text-primary' : 'text-muted hover:text-primary',
            )}
          >
            <Icon size={12} /> {label}
          </button>
        );
      })}
    </div>
  );
}
