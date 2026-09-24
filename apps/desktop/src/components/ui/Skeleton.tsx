import { cn } from '@/lib/cn';

/**
 * Bloco cinza no lugar do conteúdo que ainda vai chegar (F08-03: skeletons, não spinners).
 * A forma antecipa o layout, então nada pula quando os dados chegam. Parado com
 * `prefers-reduced-motion`.
 */
export function Skeleton({ className }: { className?: string }) {
  return (
    <div
      aria-hidden
      className={cn('animate-pulse rounded-md bg-hover motion-reduce:animate-none', className)}
    />
  );
}

/** Lista de linhas-esqueleto com um rótulo para leitor de tela. */
export function SkeletonList({
  rows,
  label,
  className,
  rowClassName = 'h-9',
}: {
  rows: number;
  label: string;
  className?: string;
  rowClassName?: string;
}) {
  return (
    <div
      role="status"
      aria-busy="true"
      aria-label={label}
      className={cn('flex flex-col gap-1.5', className)}
    >
      {Array.from({ length: rows }, (_, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: linhas idênticas e fixas
        <Skeleton key={i} className={rowClassName} />
      ))}
    </div>
  );
}
