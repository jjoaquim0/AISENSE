import { cn } from '@/lib/cn';

/** Estados de `AgentState` (docs/02-arquitetura.md). */
export type AgentState = 'idle' | 'busy' | 'awaiting_input' | 'failed' | 'stopped' | 'starting';

type Shape = 'solid' | 'half' | 'ring' | 'triangle' | 'spinner';

/** docs/08, "Indicadores de estado do agente": cor + forma + texto, nunca só cor. */
const STATES: Record<AgentState, { color: string; label: string; shape: Shape; motion: string }> = {
  idle: { color: 'text-idle', label: 'Ocioso', shape: 'solid', motion: '' },
  busy: {
    color: 'text-busy',
    label: 'Trabalhando',
    shape: 'half',
    motion: 'motion-spin-steps',
  },
  awaiting_input: {
    color: 'text-awaiting',
    label: 'Aguardando você',
    shape: 'solid',
    motion: 'motion-pulse-steps',
  },
  failed: { color: 'text-failed', label: 'Erro', shape: 'triangle', motion: '' },
  stopped: { color: 'text-stopped', label: 'Parado', shape: 'ring', motion: '' },
  starting: {
    color: 'text-busy',
    label: 'Iniciando',
    shape: 'spinner',
    motion: 'motion-spin-steps [animation-duration:1s]',
  },
};

function Glyph({ shape }: { shape: Shape }) {
  switch (shape) {
    case 'solid':
      return <circle cx="5" cy="5" r="4" fill="currentColor" />;
    case 'half':
      return (
        <>
          <circle cx="5" cy="5" r="3.5" fill="none" stroke="currentColor" strokeWidth="1.2" />
          <path d="M5 1.5a3.5 3.5 0 0 1 0 7z" fill="currentColor" />
        </>
      );
    case 'ring':
      return <circle cx="5" cy="5" r="3.5" fill="none" stroke="currentColor" strokeWidth="1.2" />;
    case 'triangle':
      return <path d="M5 1 9 9H1z" fill="currentColor" />;
    case 'spinner':
      return (
        <>
          <circle cx="5" cy="5" r="3.5" fill="none" stroke="currentColor" strokeOpacity="0.3" />
          <path
            d="M5 1.5a3.5 3.5 0 0 1 3.5 3.5"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.2"
          />
        </>
      );
  }
}

interface StatusDotProps {
  state: AgentState;
  /** Mostra o rótulo ao lado. Cor nunca é o único sinal (docs/08 — acessibilidade). */
  withLabel?: boolean;
  /**
   * `low` quando o detector decidiu só pelo silêncio, sem reconhecer a tela (docs/05).
   * O rótulo ganha "?" e o nome acessível diz que é incerto.
   */
  confidence?: 'high' | 'low';
  className?: string;
}

export function StatusDot({
  state,
  withLabel = false,
  confidence = 'high',
  className,
}: StatusDotProps) {
  const { color, label, shape, motion } = STATES[state];
  const unsure = confidence === 'low';
  const name = unsure ? `${label} (sem certeza)` : label;
  return (
    <span className={cn('inline-flex items-center gap-1.5', className)} title={name}>
      <svg
        viewBox="0 0 10 10"
        className={cn('size-2.5 shrink-0', color, motion)}
        role="img"
        aria-label={name}
      >
        <Glyph shape={shape} />
      </svg>
      {withLabel && (
        <span className="text-caption text-secondary">
          {label}
          {unsure && '?'}
        </span>
      )}
    </span>
  );
}
