import {
  Archive,
  ArchiveRestore,
  Folder,
  MoreHorizontal,
  Play,
  Plus,
  Trash2,
  Users,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import {
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
import { onAgentState } from '@/features/agents/api';
import { RuntimeList } from '@/features/runtimes/RuntimeList';
import { isDesktop } from '@/lib/api';
import { cn } from '@/lib/cn';
import type { TeamSummary } from '@/types/generated/TeamSummary';
import { errorMessage, teamsApi } from './api';
import { CreateTeamWizard } from './CreateTeamWizard';
import { DeleteTeamDialog } from './DeleteTeamDialog';
import { useTeams } from './store';

/** T2 — Início / Equipes (docs/09). */
export function TeamsHome() {
  const { teams, error, showArchived, load, setShowArchived, setWizardOpen } = useTeams();

  useEffect(() => {
    if (!isDesktop()) return;
    void load();
    // Estados dos agentes mudam sozinhos (queda, reinício); os pontos do card seguem.
    const unlisten = onAgentState(() => void load());
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [load]);

  if (teams && teams.length === 0 && !showArchived) {
    return (
      <>
        <EmptyState
          icon={<Users size={22} />}
          title="Monte sua primeira equipe"
          description="Uma equipe reúne agentes em terminais reais que conversam entre si e compartilham um quadro de trabalho."
          action={
            <div className="flex w-full flex-col items-center gap-5">
              <Button variant="primary" onClick={() => setWizardOpen(true)}>
                <Plus size={15} /> Nova equipe
              </Button>
              <RuntimeList />
            </div>
          }
        />
        <CreateTeamWizard />
      </>
    );
  }

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-4 px-6 py-6">
      <header className="flex items-center justify-between gap-4">
        <h1 className="text-display text-primary">Suas equipes</h1>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-1.5 text-caption text-secondary">
            <input
              type="checkbox"
              checked={showArchived}
              onChange={(e) => setShowArchived(e.target.checked)}
            />
            Mostrar arquivadas
          </label>
          <Button variant="primary" onClick={() => setWizardOpen(true)}>
            <Plus size={15} /> Nova equipe
          </Button>
        </div>
      </header>

      {error && <p className="text-caption text-failed">{error}</p>}
      {!teams && !error && <p className="text-caption text-muted">Carregando…</p>}

      {teams && (
        <ul className="grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] gap-3">
          {teams.map((summary) => (
            <TeamCard key={summary.team.id} summary={summary} onChanged={load} />
          ))}
          <li>
            <button
              type="button"
              onClick={() => setWizardOpen(true)}
              className="flex h-full min-h-40 w-full flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-strong text-secondary transition-colors duration-100 hover:border-accent hover:text-accent"
            >
              <Plus size={18} />
              <span className="text-body">Nova equipe</span>
              <span className="text-caption text-muted">ou use um modelo</span>
            </button>
          </li>
        </ul>
      )}
      <CreateTeamWizard />
    </div>
  );
}

function TeamCard({
  summary,
  onChanged,
}: {
  summary: TeamSummary;
  onChanged: () => Promise<void>;
}) {
  const { team, agents } = summary;
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const archived = team.archivedAt !== null;
  const running = agents.filter((a) => a.state !== 'stopped' && a.state !== 'failed').length;

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setProblem(null);
    try {
      await action();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      setBusy(false);
      await onChanged();
    }
  };

  const start = () =>
    run(async () => {
      const failures = await teamsApi.start(team.id);
      if (failures.length > 0) {
        const names = failures.map((f) => {
          const agent = agents.find((a) => a.id === f.agentId);
          return `@${agent?.handle ?? f.agentId}: ${errorMessage(f.error)}`;
        });
        setProblem(names.join('\n'));
      }
    });

  return (
    <li
      className={cn(
        'relative flex min-h-40 flex-col gap-2 overflow-hidden rounded-xl border border-subtle bg-surface p-3 pl-4',
        archived && 'opacity-70',
      )}
    >
      <span
        aria-hidden
        className="absolute inset-y-0 left-0 w-1"
        style={{ background: `var(--agent-${team.color})` }}
      />
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <h2 className="truncate text-heading text-primary">{team.name}</h2>
          {team.mission && (
            <p className="truncate text-caption text-secondary" title={team.mission}>
              {team.mission}
            </p>
          )}
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <IconButton label={`Ações da equipe ${team.name}`} size="sm">
              <MoreHorizontal size={14} />
            </IconButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onSelect={() => run(() => teamsApi.setArchived(team.id, !archived))}>
              <span className="flex items-center gap-2">
                {archived ? <ArchiveRestore size={14} /> : <Archive size={14} />}
                {archived ? 'Desarquivar' : 'Arquivar'}
              </span>
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem danger onSelect={() => setDeleting(true)}>
              <span className="flex items-center gap-2">
                <Trash2 size={14} /> Excluir…
              </span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <div className="flex items-center gap-2">
        <span className="flex items-center gap-1">
          {agents.map((agent) => (
            <StatusDot key={agent.id} state={agent.state} />
          ))}
        </span>
        <span className="text-caption text-secondary">
          {agents.length === 1 ? '1 agente' : `${agents.length} agentes`}
        </span>
      </div>
      <p className="text-caption text-muted">
        {archived ? 'arquivada' : running > 0 ? `${running} ativos` : 'parada'}
      </p>
      <p className="flex items-center gap-1 truncate text-caption text-muted" title={team.workdir}>
        <Folder size={12} className="shrink-0" />
        <span className="truncate font-mono">{team.workdir}</span>
      </p>

      {problem && (
        <p className="text-caption whitespace-pre-line text-failed" role="alert">
          {problem}
        </p>
      )}

      <div className="mt-auto flex items-center justify-between pt-1">
        <span className="text-caption text-muted">{relativeTime(team.updatedAt)}</span>
        {!archived && (
          <Button size="sm" onClick={() => void start()} disabled={busy || agents.length === 0}>
            <Play size={13} /> Iniciar equipe
          </Button>
        )}
      </div>

      <DeleteTeamDialog
        team={team}
        open={deleting}
        onOpenChange={setDeleting}
        onDeleted={() => void onChanged()}
      />
    </li>
  );
}

const relative = new Intl.RelativeTimeFormat('pt-BR', { numeric: 'auto' });

/** "há 2 minutos", "ontem" — para a última atividade do card. */
function relativeTime(epochMs: number): string {
  const seconds = Math.round((epochMs - Date.now()) / 1000);
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ['day', 86_400],
    ['hour', 3_600],
    ['minute', 60],
  ];
  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size) return relative.format(Math.round(seconds / size), unit);
  }
  return 'agora';
}
