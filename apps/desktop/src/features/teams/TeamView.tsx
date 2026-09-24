import {
  ArrowLeft,
  FileCode,
  Folder,
  NotebookPen,
  Plus,
  RotateCw,
  SquareTerminal,
} from 'lucide-react';
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Dialog, EmptyState, IconButton } from '@/components/ui';
import { AgentFormDialog } from '@/features/agents/AgentFormDialog';
import { agentsApi } from '@/features/agents/api';
import { boardApi } from '@/features/board/api';
import { NewCardDialog } from '@/features/board/NewCardDialog';
import { busApi } from '@/features/bus/api';
import { GuardBanner } from '@/features/bus/GuardBanner';
import { useUnread } from '@/features/bus/useUnread';
import { type PaletteAction, usePalette, usePaletteActions } from '@/features/palette/paletteStore';
import { ProjectCommandsDialog } from '@/features/project/ProjectCommandsDialog';
import { ProposalsBanner } from '@/features/proposals/ProposalsBanner';
import { ShellSlot } from '@/features/shell/slots';
import { usePanels } from '@/features/shell/usePanels';
import { AgentInspector } from '@/features/team-room/components/AgentInspector';
import { AgentPane } from '@/features/team-room/components/AgentPane';
import { AgentSidebar } from '@/features/team-room/components/AgentSidebar';
import { FocusView } from '@/features/team-room/components/FocusView';
import { type DragHandle, GridView } from '@/features/team-room/components/GridView';
import { PresetPicker } from '@/features/team-room/components/PresetPicker';
import { TeamControls } from '@/features/team-room/components/TeamControls';
import { ViewPicker } from '@/features/team-room/components/ViewPicker';
import { useLiveStates } from '@/features/team-room/hooks/useLiveStates';
import { useTeamLayout } from '@/features/team-room/hooks/useTeamLayout';
import type { PaneAction } from '@/features/team-room/paneMenu';
import { roomShortcuts } from '@/features/team-room/shortcuts';
import { isRunning } from '@/features/team-room/sidebar';
import { focusAgentPane } from '@/features/terminal/focus';
import { TimelineView } from '@/features/timeline/TimelineView';
import { useShortcuts } from '@/lib/useShortcuts';
import type { Agent } from '@/types/generated/Agent';
import type { BoardView } from '@/types/generated/BoardView';
import type { StartOutcome } from '@/types/generated/StartOutcome';
import type { TeamSummary } from '@/types/generated/TeamSummary';
import { describeStartReport, errorMessage, teamsApi } from './api';
import { useTeams } from './store';

// Sob demanda: o quadro e o editor de notas trazem o preview de Markdown.
const FlowView = lazy(() =>
  import('@/features/flow/FlowView').then((m) => ({ default: m.FlowView })),
);
const BoardScreen = lazy(() =>
  import('@/features/board/BoardScreen').then((m) => ({ default: m.BoardScreen })),
);
const NotesPanel = lazy(() =>
  import('@/features/notes/NotesPanel').then((m) => ({ default: m.NotesPanel })),
);

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
  const [notesOpen, setNotesOpen] = useState(false);
  const { grid, setGrid, view, setView, flowPositions, setFlowPositions } = useTeamLayout(
    team,
    agents.map((a) => a.id),
    setProblem,
  );

  const { stateOf, confidenceOf, eventsOf } = useLiveStates(summary.agents);
  const { pendingOf } = useUnread(team.id, agents);
  const showInspector = usePanels((s) => s.showInspector);

  const reloadAgents = useCallback(async () => {
    const list = await agentsApi.list(team.id);
    setAgents(list);
    setSelectedId((current) =>
      current && list.some((a) => a.id === current) ? current : (list[0]?.id ?? null),
    );
  }, [team.id]);

  // O resumo da equipe (cards da T2) é recarregado pela TeamsHome a cada `agent:state`;
  // a sidebar e os painéis não esperam por ele (`useLiveStates`).
  useEffect(() => {
    void reloadAgents().catch((e: unknown) => setProblem(errorMessage(e)));
  }, [reloadAgents]);

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

  const handleOf = (id: string) => agents.find((a) => a.id === id)?.handle ?? id;

  // Onde cada agente foi trabalhar no último start (bancada), para o cabeçalho do painel.
  const [branches, setBranches] = useState<Record<string, string>>({});
  const launched = (agentId: string, outcome: StartOutcome) => {
    const branch = outcome.workdir.bench?.branch;
    setBranches((b) => ({ ...b, [agentId]: branch ?? '' }));
    // Ressalvas do start: bancada que não deu, skills que ficaram de fora (F04-03) e o
    // que a materialização não conseguiu escrever (F04-04).
    const notes = [
      outcome.workdir.warning,
      ...outcome.skills.ignored.map((ignored) => ignored.message),
      ...outcome.notes,
    ].filter((note): note is string => Boolean(note));
    if (notes.length > 0) setNotice({ text: `@${handleOf(agentId)}: ${notes.join(' · ')}` });
  };
  const startAgent = (agentId: string) =>
    act(async () => launched(agentId, await agentsApi.start(agentId)));
  const restartAgent = (agentId: string) =>
    act(async () => launched(agentId, await agentsApi.restart(agentId)));

  // "Limpar" do menu: um contador por agente que o terminal observa.
  const [clears, setClears] = useState<Record<string, number>>({});
  const renderPane = (id: string, drag?: DragHandle) => {
    const agent = agents.find((a) => a.id === id);
    if (!agent) return null;
    return (
      <AgentPane
        agent={agent}
        state={stateOf(agent.id)}
        confidence={confidenceOf(agent.id)}
        branch={branches[agent.id]}
        focused={agent.id === selectedId}
        clearSignal={clears[agent.id]}
        dragHandle={drag}
        onFocus={() => setSelectedId(agent.id)}
        onAction={(action) => paneAction(agent, action)}
        className="min-w-0 flex-1"
      />
    );
  };

  const paneAction = (agent: Agent, action: PaneAction) => {
    switch (action) {
      case 'start':
        return void startAgent(agent.id);
      case 'restart':
        return void restartAgent(agent.id);
      case 'stop':
        return void act(() => agentsApi.stop(agent.id));
      case 'clear':
        return setClears((c) => ({ ...c, [agent.id]: (c[agent.id] ?? 0) + 1 }));
      case 'duplicate':
        return void act(async () => {
          const copy = await agentsApi.duplicate(agent.id);
          await reloadAgents();
          setSelectedId(copy.id);
          setNotice({ text: `@${agent.handle} duplicado como @${copy.handle}.` });
        });
      case 'configure':
        return setForm({ open: true, agent });
    }
  };

  // Arrastar na sidebar: a lista muda na hora e o core confirma; se recusar (outra
  // janela criou ou excluiu um agente no meio), volta ao que está gravado.
  const reorderAgents = (ids: string[]) => {
    setAgents((list) => ids.flatMap((id) => list.find((a) => a.id === id) ?? []));
    void agentsApi
      .reorder(team.id, ids)
      .then(setAgents)
      .catch((e: unknown) => {
        setProblem(errorMessage(e));
        void reloadAgents();
      });
  };

  const selected = agents.find((a) => a.id === selectedId) ?? null;

  // Diálogo T5 e aba Config do inspetor salvam pelo mesmo caminho.
  const agentSaved = (agent: Agent, restartRequired: boolean) => {
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
  };

  // F03-08: ⌘1..9, ⌘G, ⌘T, ⌘W, ⌘\ — a regra de cada um mora em `roomShortcuts`.
  const shortcuts = useMemo(
    () =>
      roomShortcuts({
        grid,
        view,
        selectedId,
        setGrid,
        setView,
        select: setSelectedId,
        focusPane: focusAgentPane,
        newAgent: () => setForm({ open: true }),
      }),
    [grid, view, selectedId, setGrid, setView],
  );
  useShortcuts(shortcuts);

  // Paleta (T10): o que dá para fazer nesta equipe, sem mouse.
  const [newCard, setNewCard] = useState<BoardView | null>(null);
  // As funções locais mudam a cada render; a paleta chama sempre a versão mais nova.
  const live = useRef({ act, startAgent, restartAgent, handleOf });
  live.current = { act, startAgent, restartAgent, handleOf };
  const paletteActions = useMemo<PaletteAction[]>(() => {
    const { act, startAgent, restartAgent, handleOf } = {
      act: (f: () => Promise<unknown>) => live.current.act(f),
      startAgent: (id: string) => live.current.startAgent(id),
      restartAgent: (id: string) => live.current.restartAgent(id),
      handleOf: (id: string) => live.current.handleOf(id),
    };
    const views: [typeof view, string][] = [
      ['grid', 'Grade'],
      ['focus', 'Foco'],
      ['flow', 'Fluxo'],
      ['timeline', 'Mensagens'],
      ['board', 'Quadro'],
    ];
    const report = (r: Awaited<ReturnType<typeof teamsApi.start>>) => {
      const lines = describeStartReport(r, handleOf);
      if (lines.length > 0) setNotice({ text: lines.join(' · ') });
    };
    return [
      ...views.map(([v, label]) => ({
        id: `view:${v}`,
        label: `Vista ${label}`,
        group: 'Vista',
        shortcut: '⌘G',
        keywords: ['vista', 'mostrar', label.toLowerCase()],
        run: () => setView(v),
      })),
      {
        id: 'team:start',
        label: `Iniciar equipe ${team.name}`,
        group: 'Equipe',
        keywords: ['play', 'ligar', 'subir'],
        run: () => act(async () => report(await teamsApi.start(team.id))),
      },
      {
        id: 'team:stop',
        label: `Parar equipe ${team.name}`,
        group: 'Equipe',
        keywords: ['desligar', 'pausar'],
        run: () => act(() => teamsApi.stop(team.id)),
      },
      {
        id: 'team:restart',
        label: `Reiniciar equipe ${team.name}`,
        group: 'Equipe',
        run: () => act(async () => report(await teamsApi.restart(team.id))),
      },
      {
        id: 'create:agent',
        label: 'Novo agente',
        group: 'Criar',
        keywords: ['criar', 'agente'],
        run: () => setForm({ open: true }),
      },
      {
        id: 'create:card',
        label: 'Novo cartão no quadro',
        group: 'Criar',
        keywords: ['criar', 'tarefa', 'task', 'quadro'],
        run: () =>
          boardApi
            .get(team.id)
            .then(setNewCard)
            .catch((e: unknown) => setProblem(errorMessage(e))),
      },
      { id: 'open:notes', label: 'Notas da equipe', group: 'Abrir', run: () => setNotesOpen(true) },
      {
        id: 'open:commands',
        label: 'Comandos do projeto',
        group: 'Abrir',
        run: () => setCommandsOpen(true),
      },
      ...agents.flatMap((a) => [
        {
          id: `agent:start:${a.id}`,
          label: `Iniciar @${a.handle}`,
          group: 'Agentes',
          keywords: ['play', 'ligar', a.name],
          run: () => startAgent(a.id),
        },
        {
          id: `agent:stop:${a.id}`,
          label: `Parar @${a.handle}`,
          group: 'Agentes',
          keywords: ['desligar', a.name],
          run: () => act(() => agentsApi.stop(a.id)),
        },
        {
          id: `agent:restart:${a.id}`,
          label: `Reiniciar @${a.handle}`,
          group: 'Agentes',
          run: () => restartAgent(a.id),
        },
        {
          id: `agent:focus:${a.id}`,
          label: `Ir para o terminal de @${a.handle}`,
          group: 'Agentes',
          keywords: ['foco', 'terminal', a.name],
          run: () => {
            setSelectedId(a.id);
            setView('focus');
          },
        },
      ]),
    ];
  }, [team.id, team.name, agents, setView]);
  usePaletteActions('team-room', paletteActions);
  const setSend = usePalette((s) => s.setSend);
  useEffect(() => {
    setSend((to, body) => busApi.send(team.id, [to], body));
    return () => setSend(null);
  }, [team.id, setSend]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      {notesOpen && (
        <Suspense fallback={null}>
          <NotesPanel teamId={team.id} teamName={team.name} onClose={() => setNotesOpen(false)} />
        </Suspense>
      )}
      <ShellSlot name="sidebar">
        <AgentSidebar
          agents={agents}
          selectedId={selectedId}
          stateOf={stateOf}
          confidenceOf={confidenceOf}
          pendingOf={pendingOf}
          onSelect={setSelectedId}
          onInspect={(id) => {
            setSelectedId(id);
            showInspector();
          }}
          onReorder={reorderAgents}
          onStart={(id) => {
            setSelectedId(id);
            void startAgent(id);
          }}
          onStop={(id) => void act(() => agentsApi.stop(id))}
          onEdit={(agent) => setForm({ open: true, agent })}
          onDelete={setDeleting}
          onAdd={() => setForm({ open: true })}
        />
      </ShellSlot>
      <ShellSlot name="inspector">
        <AgentInspector
          agent={selected}
          state={selected ? stateOf(selected.id) : 'stopped'}
          confidence={selected ? confidenceOf(selected.id) : undefined}
          events={selected ? eventsOf(selected.id) : []}
          teamWorkdir={team.workdir}
          teamId={team.id}
          siblings={agents}
          onSaved={agentSaved}
        />
      </ShellSlot>
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
        <ViewPicker value={view} onChange={setView} />
        {view === 'grid' && (
          <PresetPicker value={grid.preset} onChange={(preset) => setGrid({ ...grid, preset })} />
        )}
        <Button onClick={() => setNotesOpen(true)}>
          <NotebookPen size={13} /> Notas
        </Button>
        <Button onClick={() => setCommandsOpen(true)}>
          <FileCode size={13} /> Comandos
        </Button>
        <TeamControls
          teamId={team.id}
          running={agents.filter((a) => isRunning(stateOf(a.id))).length}
          handleOf={handleOf}
          onReport={(report) => {
            const lines = describeStartReport(report, handleOf);
            if (lines.length > 0) setNotice({ text: lines.join(' · ') });
            void load();
          }}
          onError={setProblem}
        />
        <Button variant="primary" onClick={() => setForm({ open: true })}>
          <Plus size={14} /> Novo agente
        </Button>
      </header>

      <GuardBanner teamId={team.id} />
      <ProposalsBanner
        teamId={team.id}
        agents={agents}
        onDecided={(p) => {
          // Aceitar "criar agente" muda a lista da equipe.
          if (p.state === 'accepted' && p.action.kind === 'createAgent') {
            void reloadAgents();
            void load();
          }
        }}
      />
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
                    if (agent) void restartAgent(agent.id);
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
        <section aria-label="Terminais da equipe" className="flex min-w-0 flex-1 flex-col p-2">
          {view === 'flow' ? (
            <Suspense fallback={null}>
              <FlowView
                teamId={team.id}
                agents={agents}
                stateOf={stateOf}
                positions={flowPositions}
                onPositions={setFlowPositions}
                onOpenAgent={(id) => {
                  setSelectedId(id);
                  setView('focus');
                }}
              />
            </Suspense>
          ) : view === 'board' ? (
            <Suspense fallback={null}>
              <BoardScreen teamId={team.id} />
            </Suspense>
          ) : view === 'timeline' ? (
            <TimelineView teamId={team.id} agents={agents} />
          ) : agents.length > 0 && view === 'focus' ? (
            <FocusView
              order={grid.order}
              agents={agents}
              focusedId={selectedId}
              stateOf={stateOf}
              confidenceOf={confidenceOf}
              renderPane={(id) => renderPane(id)}
              onFocus={setSelectedId}
              onAdd={() => setForm({ open: true })}
            />
          ) : agents.length > 0 ? (
            <GridView
              layout={grid}
              onChange={setGrid}
              labelOf={(id) => `@${handleOf(id)}`}
              renderPane={renderPane}
            />
          ) : (
            <EmptyState
              icon={<SquareTerminal size={22} />}
              title="Nenhum agente ainda"
              description="Crie um agente em “Novo agente” para ver o terminal dele aqui."
            />
          )}
        </section>
      </div>

      <AgentFormDialog
        teamId={team.id}
        agent={form.agent}
        siblings={agents}
        running={form.agent ? isRunning(stateOf(form.agent.id)) : false}
        open={form.open}
        onOpenChange={(open) => setForm((f) => ({ ...f, open }))}
        onSaved={agentSaved}
      />

      {newCard && (
        <NewCardDialog
          open
          teamId={team.id}
          columns={newCard.columns}
          agents={newCard.agents}
          onOpenChange={(open) => !open && setNewCard(null)}
          onCreated={() => setNotice({ text: 'Cartão criado no quadro.' })}
        />
      )}

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
