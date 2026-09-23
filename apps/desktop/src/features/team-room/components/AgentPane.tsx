import { GitBranch, MoreVertical, Play, SquareTerminal } from 'lucide-react';
import {
  Badge,
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  EmptyState,
  IconButton,
  StatusDot,
} from '@/components/ui';
import { Terminal } from '@/features/terminal/Terminal';
import { cn } from '@/lib/cn';
import type { Agent } from '@/types/generated/Agent';
import type { AgentState } from '@/types/generated/AgentState';
import type { StateConfidence } from '@/types/generated/StateConfidence';
import { isRunning, type PaneAction, paneMenu } from '../paneMenu';

interface AgentPaneProps {
  agent: Agent;
  state: AgentState;
  confidence?: StateConfidence;
  /** Branch da bancada, quando o agente está numa. */
  branch?: string;
  /** Mensagens na caixa de entrada (o barramento chega na Fase 05). */
  pending?: number;
  focused?: boolean;
  /** Muda para pedir ao terminal que limpe a tela. */
  clearSignal?: number;
  onAction: (action: PaneAction) => void;
  onFocus?: () => void;
  className?: string;
}

/**
 * Um agente na Sala da Equipe (docs/09, T4.1): borda de 3px na cor do agente — a âncora
 * visual com 9 terminais na tela —, cabeçalho com handle, runtime, estado e menu `⋮`,
 * e o terminal. Parado, mostra um estado vazio com ação em vez de um retângulo preto.
 */
export function AgentPane({
  agent,
  state,
  confidence = 'high',
  branch,
  pending = 0,
  focused = false,
  clearSignal,
  onAction,
  onFocus,
  className,
}: AgentPaneProps) {
  const running = isRunning(state);
  // Uma sessão que já existiu (inclusive a que acabou de cair) tem histórico no core.
  const hasSession = running || state === 'failed';

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: foco do painel por clique; o teclado usa ⌘1..9 (F03-08)
    <div
      data-focused={focused || undefined}
      onMouseDown={onFocus}
      className={cn(
        'flex min-h-0 flex-col overflow-hidden rounded-lg border border-l-[3px] bg-surface',
        focused ? 'border-ring' : 'border-subtle',
        className,
      )}
      style={{ borderLeftColor: `var(--agent-${agent.color})` }}
    >
      <div
        className={cn(
          'flex items-center gap-2 border-b border-subtle px-3 py-1.5',
          focused && 'bg-hover',
        )}
      >
        <span className="truncate text-label text-primary">@{agent.handle}</span>
        <span className="truncate text-caption text-muted">{agent.adapterId}</span>
        <StatusDot state={state} confidence={confidence} withLabel className="ml-1" />
        {pending > 0 && (
          <Badge variant="accent" className="tabular-nums">
            {pending}
          </Badge>
        )}
        <span className="ml-auto flex items-center gap-2">
          {branch && (
            <span
              className="flex items-center gap-1 font-mono text-caption text-muted"
              title="Bancada"
            >
              <GitBranch size={11} /> {branch}
            </span>
          )}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <IconButton label={`Ações de @${agent.handle}`} size="sm">
                <MoreVertical size={13} />
              </IconButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
              {paneMenu(state).map((item) => (
                <PaneMenuEntry key={item.action} {...item} onAction={onAction} />
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </span>
      </div>
      {hasSession ? (
        <Terminal
          key={agent.id}
          agentId={agent.id}
          clearSignal={clearSignal}
          className="min-h-0 flex-1"
        />
      ) : (
        <EmptyState
          icon={<SquareTerminal size={22} />}
          title="Agente parado"
          description={agent.role || 'Inicie o agente para abrir o terminal dele.'}
          action={
            <Button variant="primary" onClick={() => onAction('start')}>
              <Play size={13} /> Iniciar
            </Button>
          }
        />
      )}
    </div>
  );
}

function PaneMenuEntry({
  action,
  label,
  disabled,
  danger,
  onAction,
}: ReturnType<typeof paneMenu>[number] & { onAction: (action: PaneAction) => void }) {
  return (
    <>
      {action === 'duplicate' && <DropdownMenuSeparator />}
      <DropdownMenuItem disabled={disabled} danger={danger} onSelect={() => onAction(action)}>
        {label}
      </DropdownMenuItem>
    </>
  );
}
