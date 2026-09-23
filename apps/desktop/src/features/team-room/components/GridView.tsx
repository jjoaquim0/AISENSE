import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  DragOverlay,
  type DragStartEvent,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import {
  rectSortingStrategy,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Eye } from 'lucide-react';
import { type HTMLAttributes, type ReactNode, useMemo, useRef, useState } from 'react';
import { cn } from '@/lib/cn';
import {
  FREE_UNITS,
  type FreeRect,
  freeRectOf,
  type GridLayout,
  hiddenIds,
  PRESET_SHAPE,
  promote,
  reorder,
  setFreeRect,
  visibleIds,
} from '../gridLayout';

/** O que o painel precisa aplicar no cabeçalho para ser arrastado por ele. */
export interface DragHandle {
  setRef?: (element: HTMLElement | null) => void;
  props: HTMLAttributes<HTMLElement>;
}

interface GridViewProps {
  layout: GridLayout;
  onChange: (layout: GridLayout) => void;
  renderPane: (id: string, drag: DragHandle) => ReactNode;
  /** Rótulo curto do painel (ex.: `@backend`) para a sombra do arraste e os ocultos. */
  labelOf: (id: string) => string;
}

/**
 * Vista Grid (docs/09, T4.1). Nos presets, arrastar pelo cabeçalho troca painéis de
 * lugar; no modo livre, arrasta e redimensiona numa grade invisível de 24×24.
 *
 * Desempenho com 9 terminais: durante o arraste nada re-renderiza terminal. Nos presets
 * os painéis andam só por `transform` (GPU) e o que segue o cursor é uma sombra leve,
 * não o terminal; no modo livre o movimento é aplicado direto no DOM e o estado só
 * muda ao soltar.
 */
export function GridView({ layout, onChange, renderPane, labelOf }: GridViewProps) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      {layout.preset === 'free' ? (
        <FreeGrid layout={layout} onChange={onChange} renderPane={renderPane} />
      ) : (
        <PresetGrid layout={layout} onChange={onChange} renderPane={renderPane} labelOf={labelOf} />
      )}
      <HiddenPanes layout={layout} onChange={onChange} labelOf={labelOf} />
    </div>
  );
}

// ───────────────────────────── presets ─────────────────────────────

function PresetGrid({ layout, onChange, renderPane, labelOf }: GridViewProps) {
  const [dragging, setDragging] = useState<string | null>(null);
  const sensors = useSensors(
    // A distância mínima deixa o clique no menu ⋮ do cabeçalho continuar sendo clique.
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const ids = visibleIds(layout);
  const [cols, rows] = PRESET_SHAPE[layout.preset as keyof typeof PRESET_SHAPE];

  const onDragStart = (event: DragStartEvent) => setDragging(String(event.active.id));
  const onDragEnd = (event: DragEndEvent) => {
    setDragging(null);
    if (event.over && event.active.id !== event.over.id) {
      onChange({
        ...layout,
        order: reorder(layout.order, String(event.active.id), String(event.over.id)),
      });
    }
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onDragCancel={() => setDragging(null)}
    >
      <SortableContext items={ids} strategy={rectSortingStrategy}>
        <div
          className="grid min-h-0 flex-1 gap-2"
          style={{
            gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
            gridTemplateRows: `repeat(${rows}, minmax(0, 1fr))`,
          }}
        >
          {ids.map((id) => (
            <SortablePane key={id} id={id} renderPane={renderPane} />
          ))}
        </div>
      </SortableContext>
      <DragOverlay dropAnimation={null}>
        {dragging && (
          <div className="flex h-10 items-center rounded-lg border border-ring bg-raised px-3 text-label text-primary shadow-md">
            {labelOf(dragging)}
          </div>
        )}
      </DragOverlay>
    </DndContext>
  );
}

function SortablePane({ id, renderPane }: { id: string; renderPane: GridViewProps['renderPane'] }) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id });
  // O dnd-kit re-renderiza cada item a cada mudança de alvo durante o arraste; o
  // conteúdo (terminal incluído) só precisa renderizar de novo se algo dele mudou.
  const pane = useMemo(
    () =>
      renderPane(id, {
        setRef: setActivatorNodeRef,
        props: { ...attributes, ...listeners, 'aria-roledescription': 'painel arrastável' },
      }),
    [renderPane, id, setActivatorNodeRef, attributes, listeners],
  );
  return (
    <div
      ref={setNodeRef}
      className={cn('flex min-h-0 min-w-0', isDragging && 'opacity-40')}
      style={{ transform: CSS.Translate.toString(transform), transition }}
    >
      {pane}
    </div>
  );
}

// ───────────────────────────── livre ─────────────────────────────

function FreeGrid({
  layout,
  onChange,
  renderPane,
}: Pick<GridViewProps, 'layout' | 'onChange' | 'renderPane'>) {
  const area = useRef<HTMLDivElement>(null);
  return (
    <div ref={area} className="relative min-h-0 flex-1">
      {layout.order.map((id) => (
        <FreePane
          key={id}
          rect={freeRectOf(layout, id)}
          area={area}
          onCommit={(rect) => onChange(setFreeRect(layout, id, rect))}
        >
          {(drag) => renderPane(id, drag)}
        </FreePane>
      ))}
    </div>
  );
}

const pct = (units: number) => `${(units / FREE_UNITS) * 100}%`;

function FreePane({
  rect,
  area,
  onCommit,
  children,
}: {
  rect: FreeRect;
  area: React.RefObject<HTMLDivElement | null>;
  onCommit: (rect: FreeRect) => void;
  children: (drag: DragHandle) => ReactNode;
}) {
  const box = useRef<HTMLDivElement>(null);

  /** Um gesto: acompanha o ponteiro mexendo só no DOM; o estado muda ao soltar. */
  const gesture = (kind: 'move' | 'resize') => (event: React.PointerEvent<HTMLElement>) => {
    if (event.button !== 0 || !box.current || !area.current) return;
    // Clique em botão do cabeçalho (menu ⋮) não é arraste.
    if ((event.target as HTMLElement).closest('button') && kind === 'move') return;
    event.preventDefault();
    const el = box.current;
    // O gesto mexe direto no estilo; ao soltar, tudo volta ao que o React tinha posto.
    // Apagar em vez de restaurar quebra o painel: o React só reaplica o que mudou,
    // então uma largura que não mudou ficaria vazia.
    const original = {
      transform: el.style.transform,
      width: el.style.width,
      height: el.style.height,
      zIndex: el.style.zIndex,
    };
    // Durante o gesto o painel passa por cima de todos.
    el.style.zIndex = '1000';
    const bounds = area.current.getBoundingClientRect();
    const cellW = bounds.width / FREE_UNITS;
    const cellH = bounds.height / FREE_UNITS;
    const start = { x: event.clientX, y: event.clientY };
    const startW = el.offsetWidth;
    const startH = el.offsetHeight;
    let dx = 0;
    let dy = 0;

    const move = (e: PointerEvent) => {
      dx = e.clientX - start.x;
      dy = e.clientY - start.y;
      if (kind === 'move') {
        el.style.transform = `translate3d(${dx}px, ${dy}px, 0)`;
      } else {
        el.style.width = `${Math.max(cellW * 2, startW + dx)}px`;
        el.style.height = `${Math.max(cellH * 2, startH + dy)}px`;
      }
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      Object.assign(el.style, original);
      const du = Math.round(dx / cellW);
      const dv = Math.round(dy / cellH);
      if (du === 0 && dv === 0) return;
      onCommit(
        kind === 'move'
          ? { ...rect, x: rect.x + du, y: rect.y + dv }
          : { ...rect, w: rect.w + du, h: rect.h + dv },
      );
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };

  return (
    <div
      ref={box}
      className="absolute flex p-1 will-change-transform"
      style={{
        left: pct(rect.x),
        top: pct(rect.y),
        width: pct(rect.w),
        height: pct(rect.h),
        zIndex: rect.z ?? 0,
      }}
    >
      {children({ props: { onPointerDown: gesture('move') } })}
      <span
        aria-hidden
        onPointerDown={gesture('resize')}
        className="absolute right-1 bottom-1 size-3 cursor-nwse-resize rounded-br-lg border-r-2 border-b-2 border-strong"
      />
    </div>
  );
}

// ───────────────────────────── ocultos ─────────────────────────────

function HiddenPanes({
  layout,
  onChange,
  labelOf,
}: Pick<GridViewProps, 'layout' | 'onChange' | 'labelOf'>) {
  const hidden = hiddenIds(layout);
  if (hidden.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5 text-caption text-muted">
      <span>Fora da grade:</span>
      {hidden.map((id) => (
        <button
          key={id}
          type="button"
          onClick={() => onChange(promote(layout, id))}
          className="inline-flex items-center gap-1 rounded-md border border-subtle px-1.5 py-0.5 text-secondary hover:bg-hover"
        >
          <Eye size={11} /> {labelOf(id)}
        </button>
      ))}
    </div>
  );
}
