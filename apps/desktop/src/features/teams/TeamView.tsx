import {
  ArrowLeft,
  FileCode,
  Folder,
  Pencil,
  Play,
  Plus,
  RotateCw,
  Square,
  SquareTerminal,
  Trash2,
} from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button, Dialog, EmptyState, IconButton, StatusDot, Tooltip } from '@/components/ui';
import { AgentFormDialog } from '@/features/agents/AgentFormDialog';
import { agentsApi, onAgentState } from '@/features/agents/api';
import { ProjectCommandsDialog } from '@/features/project/ProjectCommandsDialog';
import { Terminal } from '@/features/terminal/Terminal';
import { cn } from '@/lib/cn';
import type { Agent } from '@/types/generated/Agent';
import type { AgentState } from '@/types/generated/AgentState';
import type { TeamSummary } from '@/types/generated/TeamSummary';
import { errorMessage, teamsApi } from './api';
import { useTeams } from './store';

const RUNNING: AgentState[] = ['starting', 'idle', 'busy', 'awaiting_input'];

/**
 * Uma equipe aberta: os agentes, seus controles e o terminal do selecionado.
 * É o degrau até a Sala da Equipe da Fase 03 (grade, foco, fluxo, linha do tempo).
 */
export function TeamView({ summary }: { summary: TeamSummary }) {
  const { team } = summary;
  const { selectTeam, load } = useTeams();
  const [agents, setAgents] = useState<Agent[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [form, setForm] = useState<{ open: boolean; agent?: Agent }>({ open: false });
  const [deleting, setDeleting] = useState<Agent | null>(null);
  const [notice, setNotice] = useState<{ text: string; restart?: Agent } | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [commandsOpen, setCommandsOpen] = useState(false);

  const stateOf = useCallback(
    (id: string): AgentState => summary.agents.find((a) => a.id === id)?.state ?? 'stopped',
    [summary.agents],
  );

  const reloadAgents = useCallback(async () => {
    const list = await agentsApi.list(team.id);
    setAgents(list);
    setSelectedId((current) =>
      current && list.some((a) => a.id === current) ? current : (list[0]?.id ?? null),
    );
  }, [team.id]);

  useEffect(() => {
    void reloadAgents().catch((e: unknown) => setProblem(errorMessage(e)));
    const unlisten = onAgentState(() => void load());
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [reloadAgents, load]);

  const act = async (action: () => Promise<unknown>) => {
    setProblem(null);
    try {
      await action();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      await load();
    }
  };

  const selected = agents.find((a) => a.id === selectedId) ?? null;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex items-center gap-3 border-b border-subtle px-4 py-2.5">
        <IconButton label="Voltar para as equipes" onClick={() => selectTeam(null)}>
          <ArrowLeft size={15} />
        </IconButton>
        <span
          aria-hidden
          className="size-3 shrink-0 rounded-sm"
          style={{ background: `var(--agent-${team.color})` }}
        />
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-heading text-primary">{team.name}</h1>
          <p className="flex items-center gap-1 truncate text-caption text-muted">
            <Folder size={11} /> <span className="font-mono">{team.workdir}</span>
          </p>
        </div>
        <Button onClick={() => setCommandsOpen(true)}>
          <FileCode size={13} /> Comandos
        </Button>
        <Button onClick={() => void act(() => teamsApi.start(team.id))}>
          <Play size={13} /> Iniciar equipe
        </Button>
        <Button variant="primary" onClick={() => setForm({ open: true })}>
          <Plus size={14} /> Novo agente
        </Button>
      </header>

      {(problem || notice) && (
        <div className="flex flex-col gap-1 border-b border-subtle px-4 py-2">
          {problem && (
            <p className="text-caption text-failed" role="alert">
              {problem}
            </p>
          )}
          {notice && (
            <p className="flex items-center gap-2 text-caption text-secondary">
              {notice.text}
              {notice.restart && (
                <Button
                  size="sm"
                  onClick={() => {
                    const agent = notice.restart;
                    setNotice(null);
                    if (agent) void act(() => agentsApi.restart(agent.id));
                  }}
                >
                  <RotateCw size={12} /> Reiniciar agora
                </Button>
              )}
            </p>
          )}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        <ul
          aria-label="Agentes"
          className="w-72 shrink-0 overflow-y-auto border-r border-subtle p-2"
        >
          {agents.length === 0 && (
            <li className="px-2 py-3 text-caption text-muted">
              Nenhum agente ainda. Crie um em “Novo agente”.
            </li>
          )}
          {agents.map((agent) => {
            const state = stateOf(agent.id);
            const running = RUNNING.includes(state);
            return (
              <li key={agent.id}>
                <div
                  className={cn(
                    'group relative flex items-center gap-2 rounded-lg py-1.5 pr-1 pl-3',
                    agent.id === selectedId ? 'bg-active' : 'hover:bg-hover',
                  )}
                >
                  <span
                    aria-hidden
                    className="absolute inset-y-1 left-0 w-[3px] rounded-full"
                    style={{ background: `var(--agent-${agent.color})` }}
                  />
                  <button
                    type="button"
                    onClick={() => setSelectedId(agent.id)}
                    className="flex min-w-0 flex-1 flex-col items-start text-left"
                  >
                    <span className="truncate text-body text-primary">@{agent.handle}</span>
                    <span className="flex items-center gap-1.5">
                      <StatusDot state={state} withLabel />
                      <span className="text-caption text-muted">· {agent.adapterId}</span>
                    </span>
                  </button>
                  {running ? (
                    <Tooltip content="Parar">
                      <IconButton
                        label={`Parar @${agent.handle}`}
                        size="sm"
                        onClick={() => void act(() => agentsApi.stop(agent.id))}
                      >
                        <Square size={12} />
                      </IconButton>
                    </Tooltip>
                  ) : (
                    <Tooltip content="Iniciar">
                      <IconButton
                        label={`Iniciar @${agent.handle}`}
                        size="sm"
                        onClick={() => {
                          setSelectedId(agent.id);
                          void act(() => agentsApi.start(agent.id));
                        }}
                      >
                        <Play size={12} />
                      </IconButton>
                    </Tooltip>
                  )}
                  <Tooltip content="Editar">
                    <IconButton
                      label={`Editar @${agent.handle}`}
                      size="sm"
                      onClick={() => setForm({ open: true, agent })}
                    >
                      <Pencil size={12} />
                    </IconButton>
                  </Tooltip>
                  <Tooltip content="Excluir">
                    <IconButton
                      label={`Excluir @${agent.handle}`}
                      size="sm"
                      variant="danger"
                      onClick={() => setDeleting(agent)}
                    >
                      <Trash2 size={12} />
                    </IconButton>
                  </Tooltip>
                </div>
              </li>
            );
          })}
        </ul>

        <section aria-label="Terminal do agente" className="flex min-w-0 flex-1 flex-col">
          {selected ? (
            <AgentPane
              agent={selected}
              state={stateOf(selected.id)}
              onStart={() => void act(() => agentsApi.start(selected.id))}
              onRestart={() => void act(() => agentsApi.restart(selected.id))}
            />
          ) : (
            <EmptyState
              icon={<SquareTerminal size={22} />}
              title="Nenhum agente selecionado"
              description="Escolha um agente à esquerda para ver o terminal dele."
            />
          )}
        </section>
      </div>

      <AgentFormDialog
        teamId={team.id}
        agent={form.agent}
        siblings={agents}
        running={form.agent ? RUNNING.includes(stateOf(form.agent.id)) : false}
        open={form.open}
        onOpenChange={(open) => setForm((f) => ({ ...f, open }))}
        onSaved={(agent, restartRequired) => {
          void reloadAgents();
          void load();
          setSelectedId(agent.id);
          setNotice(
            restartRequired
              ? {
                  text: `@${agent.handle} foi salvo. As mudanças valem no próximo início.`,
                  restart: agent,
                }
              : { text: `@${agent.handle} foi salvo.` },
          );
        }}
      />

      <ProjectCommandsDialog
        workdir={team.workdir}
        open={commandsOpen}
        onOpenChange={setCommandsOpen}
      />

      {deleting && (
        <Dialog
          open
          onOpenChange={(open) => !open && setDeleting(null)}
          title={`Excluir @${deleting.handle}?`}
          description="O agente é parado e removido da equipe, junto com o histórico de sessões."
          footer={
            <>
              <Button variant="ghost" onClick={() => setDeleting(null)}>
                Cancelar
              </Button>
              <Button
                variant="danger"
                onClick={() => {
                  const agent = deleting;
                  setDeleting(null);
                  void act(async () => {
                    await agentsApi.remove(agent.id);
                    await reloadAgents();
                  });
                }}
              >
                Excluir agente
              </Button>
            </>
          }
        />
      )}
    </div>
  );
}

function AgentPane({
  agent,
  state,
  onStart,
  onRestart,
}: {
  agent: Agent;
  state: AgentState;
  onStart: () => void;
  onRestart: () => void;
}) {
  const running = RUNNING.includes(state);
  // Uma sessão que já existiu (inclusive a que acabou de cair) tem histórico no core.
  const hasSession = running || state === 'failed';

  return (
    <>
      <div className="flex items-center gap-2 border-b border-subtle px-3 py-1.5">
        <span
          aria-hidden
          className="size-2.5 rounded-full"
          style={{ background: `var(--agent-${agent.color})` }}
        />
        <span className="text-label text-primary">@{agent.handle}</span>
        <span className="text-caption text-muted">— {agent.name}</span>
        <span className="ml-auto flex items-center gap-2">
          <StatusDot state={state} withLabel />
          {running && (
            <IconButton label={`Reiniciar @${agent.handle}`} size="sm" onClick={onRestart}>
              <RotateCw size={12} />
            </IconButton>
          )}
        </span>
      </div>
      {hasSession ? (
        <Terminal key={agent.id} agentId={agent.id} className="min-h-0 flex-1" />
      ) : (
        <EmptyState
          icon={<SquareTerminal size={22} />}
          title="Agente parado"
          description={agent.role || 'Inicie o agente para abrir o terminal dele.'}
          action={
            <Button variant="primary" onClick={onStart}>
              <Play size={13} /> Iniciar
            </Button>
          }
        />
      )}
    </>
  );
}
