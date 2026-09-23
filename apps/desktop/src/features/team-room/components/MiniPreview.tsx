import { StatusDot } from '@/components/ui';
import { cn } from '@/lib/cn';
import type { Agent } from '@/types/generated/Agent';
import type { AgentState } from '@/types/generated/AgentState';
import type { StateConfidence } from '@/types/generated/StateConfidence';

interface MiniPreviewProps {
  agent: Agent;
  state: AgentState;
  confidence?: StateConfidence;
  /** Últimas linhas da tela, já sem ANSI (vêm do core). */
  lines: string[];
  /** Posição na tira, para o atalho ⌘1..9. */
  index: number;
  onSelect: () => void;
}

/**
 * Miniatura da vista Foco (docs/09, T4.2). É texto puro, de propósito: nove xterm
 * pequenos custariam quase o mesmo que nove grandes. O conteúdo vem da tela que o
 * detector de estado já mantém no core.
 */
export function MiniPreview({
  agent,
  state,
  confidence,
  lines,
  index,
  onSelect,
}: MiniPreviewProps) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-label={`Focar @${agent.handle}`}
      className={cn(
        'flex h-full w-52 shrink-0 flex-col overflow-hidden rounded-lg border border-subtle border-l-[3px]',
        'bg-surface text-left hover:bg-hover focus-visible:outline-2 focus-visible:outline-ring',
      )}
      style={{ borderLeftColor: `var(--agent-${agent.color})` }}
    >
      <span className="flex w-full items-center gap-1.5 border-b border-subtle px-2 py-1">
        <span className="truncate text-caption text-primary">@{agent.handle}</span>
        <StatusDot state={state} confidence={confidence} className="ml-auto" />
        {index < 9 && <span className="text-caption text-muted tabular-nums">⌘{index + 1}</span>}
      </span>
      <span className="flex min-h-0 w-full flex-1 flex-col justify-end overflow-hidden bg-terminal px-2 py-1 font-mono text-[10px] leading-tight text-secondary">
        {lines.length === 0 ? (
          <span className="text-muted">—</span>
        ) : (
          lines.map((line, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: linhas de tela não têm identidade própria
            <span key={i} className="truncate whitespace-pre">
              {line}
            </span>
          ))
        )}
      </span>
    </button>
  );
}
