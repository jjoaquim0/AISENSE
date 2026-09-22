import { cn } from '@/lib/cn';

/** Estados de `AgentState` (docs/02-arquitetura.md). */
export type AgentState = 'idle' | 'busy' | 'awaiting_input' | 'failed' | 'stopped' | 'starting';

const STATES: Record<AgentState, { color: string; label: string; animate: string }> = {
  idle: { color: 'bg-idle', label: 'Ocioso', animate: '' },
  busy: { color: 'bg-busy', label: 'Trabalhando', animate: 'animate-pulse' },
  awaiting_input: { color: 'bg-awaiting', label: 'Aguardando você', animate: 'animate-pulse' },
  failed: { color: 'bg-failed', label: 'Erro', animate: '' },
  stopped: { color: 'bg-stopped', label: 'Parado', animate: '' },
  starting: { color: 'bg-busy', label: 'Iniciando', animate: 'animate-pulse' },
};

interface StatusDotProps {
  state: AgentState;
  /** Mostra o rótulo ao lado. Cor nunca é o único sinal (docs/08 — acessibilidade). */
  withLabel?: boolean;
  className?: string;
}

export function StatusDot({ state, withLabel = false, className }: StatusDotProps) {
  const { color, label, animate } = STATES[state];
  return (
    <span className={cn('inline-flex items-center gap-1.5', className)}>
      <span
        className={cn('size-2 shrink-0 rounded-full', color, animate)}
        role="img"
        aria-label={label}
      />
      {withLabel && <span className="text-caption text-secondary">{label}</span>}
    </span>
  );
}
