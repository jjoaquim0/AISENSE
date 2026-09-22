import { useCallback, useRef } from 'react';
import { cn } from '@/lib/cn';

interface ResizeHandleProps {
  /** Largura atual do painel, em px. */
  value: number;
  onChange: (width: number) => void;
  /** De que lado do painel a alça fica: define o sinal do arrasto. */
  edge: 'right' | 'left';
  min: number;
  max: number;
  label: string;
  /** Passo do ajuste por teclado. */
  step?: number;
}

/**
 * Alça de redimensionamento com `role="separator"`.
 *
 * Arrastar com o mouse é o caminho óbvio, mas as setas do teclado também ajustam —
 * sem isso o painel é inalcançável para quem não usa mouse (docs/08 — acessibilidade).
 */
export function ResizeHandle({
  value,
  onChange,
  edge,
  min,
  max,
  label,
  step = 16,
}: ResizeHandleProps) {
  const startRef = useRef<{ x: number; width: number } | null>(null);

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      event.currentTarget.setPointerCapture(event.pointerId);
      startRef.current = { x: event.clientX, width: value };
    },
    [value],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const start = startRef.current;
      if (!start) return;
      const delta = event.clientX - start.x;
      onChange(start.width + (edge === 'right' ? delta : -delta));
    },
    [edge, onChange],
  );

  const handlePointerUp = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    event.currentTarget.releasePointerCapture(event.pointerId);
    startRef.current = null;
  }, []);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      const direction = event.key === 'ArrowLeft' ? -1 : event.key === 'ArrowRight' ? 1 : 0;
      if (direction === 0) return;
      event.preventDefault();
      onChange(value + direction * step * (edge === 'right' ? 1 : -1));
    },
    [edge, onChange, step, value],
  );

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={Math.round(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onKeyDown={handleKeyDown}
      className={cn(
        'group relative w-px shrink-0 cursor-col-resize bg-subtle',
        'transition-colors duration-100 hover:bg-accent focus-visible:bg-accent',
      )}
    >
      {/* Alvo de clique maior que a linha de 1px, sem ocupar espaço no layout. */}
      <span className="absolute inset-y-0 -left-1 w-3" />
    </div>
  );
}

// Nota: `a11y/useSemanticElements` está desligada para este arquivo em `biome.json`.
// A regra sugere trocar `role="separator"` por `<hr>`, mas `<hr>` não recebe foco nem
// eventos de ponteiro. O padrão ARIA para um divisor redimensionável é exatamente o
// que está acima: role="separator" com tabindex, aria-valuenow e ajuste por setas.
