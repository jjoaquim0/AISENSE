import '@xyflow/react/dist/style.css';
import {
  Background,
  type Edge,
  type EdgeTypes,
  Handle,
  type Node,
  type NodeProps,
  type NodeTypes,
  Position,
  ReactFlow,
} from '@xyflow/react';
import { Hash, LayoutDashboard, Pause, Play } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { type AgentState, Button, StatusDot } from '@/components/ui';
import { boardApi, onBoardChanged } from '@/features/board/api';
import { shortId } from '@/features/board/boardModel';
import { CardDetailPanel } from '@/features/board/CardDetailPanel';
import { busApi, onBusMessage } from '@/features/bus/api';
import { formatCountdown, mergeMessages } from '@/features/timeline/timelineModel';
import type { Agent } from '@/types/generated/Agent';
import type { BoardView } from '@/types/generated/BoardView';
import type { CardView } from '@/types/generated/CardView';
import type { ChannelInfo } from '@/types/generated/ChannelInfo';
import type { MessageView } from '@/types/generated/MessageView';
import type { TeamId } from '@/types/generated/TeamId';
import {
  aggregateEdges,
  autoLayout,
  FLOW_WINDOWS,
  LIVE_MS,
  lastSaid,
  mergePositions,
  type Point,
  strokeWidth,
} from './flowModel';

const STATE_LABEL: Record<string, string> = {
  stopped: 'parado',
  starting: 'iniciando',
  idle: 'ocioso',
  busy: 'trabalhando',
  awaiting_input: 'aguardando você',
  failed: 'caiu',
};

type AgentData = {
  agent: Agent;
  state: AgentState;
  preview: string | null;
  card: CardView | null;
};
type ChannelData = { slug: string; count: number; members: number };
type CardData = { card: CardView };

function AgentNode({ data }: NodeProps<Node<AgentData>>) {
  const { agent, state, preview } = data;
  return (
    <div
      className="w-52 rounded-lg border border-l-[3px] border-subtle bg-surface px-2.5 py-1.5 shadow-sm"
      style={{ borderLeftColor: `var(--agent-${agent.color})` }}
      title="Duplo clique abre o terminal"
    >
      <Handle type="target" position={Position.Top} className="opacity-0" />
      <div className="flex items-center gap-1.5 text-label text-primary">
        <StatusDot state={state} />@{agent.handle}
      </div>
      <p className="text-caption text-muted">{STATE_LABEL[state] ?? state}</p>
      {preview && <p className="mt-0.5 line-clamp-2 text-caption text-secondary">{preview}</p>}
      <Handle type="source" position={Position.Bottom} className="opacity-0" />
    </div>
  );
}

function ChannelNode({ data }: NodeProps<Node<ChannelData>>) {
  return (
    <div
      className="flex h-20 w-28 flex-col items-center justify-center bg-raised text-center"
      style={{ clipPath: 'polygon(25% 0, 75% 0, 100% 50%, 75% 100%, 25% 100%, 0 50%)' }}
    >
      <Handle type="target" position={Position.Left} className="opacity-0" />
      <span className="flex items-center text-label text-primary">
        <Hash size={11} />
        {data.slug}
      </span>
      <span className="text-caption text-muted">
        {data.count} {data.count === 1 ? 'mensagem' : 'mensagens'}
      </span>
      <Handle type="source" position={Position.Right} className="opacity-0" />
    </div>
  );
}

function CardNode({ data }: NodeProps<Node<CardData>>) {
  return (
    <div className="w-44 rounded-md border border-dashed border-strong bg-base px-2 py-1 text-caption">
      <Handle type="target" position={Position.Left} className="opacity-0" />
      <span className="text-muted">{shortId(data.card.id)}</span>
      <p className="line-clamp-2 text-primary">{data.card.title}</p>
    </div>
  );
}

export const nodeTypes: NodeTypes = { agent: AgentNode, channel: ChannelNode, card: CardNode };
const edgeTypes: EdgeTypes = {};

/** O cartão em andamento de cada agente (coluna `active`, o mais recente). */
export function activeCards(board: BoardView | null): Map<string, CardView> {
  const out = new Map<string, CardView>();
  if (!board) return out;
  const active = new Set(board.columns.filter((c) => c.kind === 'active').map((c) => c.id));
  for (const card of board.cards) {
    if (!card.assignee || !active.has(card.columnId)) continue;
    const current = out.get(card.assignee);
    if (!current || card.columnSince > current.columnSince) out.set(card.assignee, card);
  }
  return out;
}

/**
 * Vista Fluxo (T4.3, F07-03/04): agentes e canais como nós, mensagens como arestas que animam
 * quando trafegam, tracejadas com contagem regressiva para pergunta pendente e vermelhas para
 * entrega falha. O cartão em andamento aparece ao lado do agente; clicar abre o detalhe.
 */
export function FlowView({
  teamId,
  agents,
  stateOf,
  positions,
  onPositions,
  onOpenAgent,
}: {
  teamId: TeamId;
  agents: Agent[];
  stateOf: (id: string) => AgentState;
  positions: Record<string, Point>;
  onPositions: (next: Record<string, Point>) => void;
  onOpenAgent: (id: string) => void;
}) {
  const [messages, setMessages] = useState<MessageView[]>([]);
  const [channels, setChannels] = useState<ChannelInfo[]>([]);
  const [board, setBoard] = useState<BoardView | null>(null);
  const [windowMs, setWindowMs] = useState<number>(FLOW_WINDOWS[0].ms);
  const [frozen, setFrozen] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  const [dragging, setDragging] = useState<Record<string, Point>>({});
  const [openCard, setOpenCard] = useState<string | null>(null);
  const frozenRef = useRef(frozen);
  frozenRef.current = frozen;

  const loadBoard = useCallback(
    () =>
      boardApi
        .get(teamId)
        .then(setBoard)
        .catch(() => {}),
    [teamId],
  );

  useEffect(() => {
    busApi
      .timeline(teamId, undefined, 500)
      .then((page) => setMessages((current) => mergeMessages(current, page)))
      .catch(() => {});
    busApi
      .channels(teamId)
      .then(setChannels)
      .catch(() => {});
    void loadBoard();
    const offMessage = onBusMessage((event) => {
      if (event.teamId !== teamId || frozenRef.current) return;
      setMessages((current) => mergeMessages(current, [event.message]));
      setNow(Date.now());
    });
    // Cartão movido no quadro (UI, CLI ou automação): o canvas acompanha na hora.
    const offBoard = onBoardChanged((event) => {
      if (event.teamId === teamId && !frozenRef.current) void loadBoard();
    });
    return () => {
      void offMessage.then((stop) => stop());
      void offBoard.then((stop) => stop());
    };
  }, [teamId, loadBoard]);

  // Relógio: animação "ao vivo" e contagem das perguntas. Congelado, para.
  useEffect(() => {
    if (frozen) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [frozen]);

  const handles = useMemo(() => agents.map((a) => a.handle), [agents]);
  const edges = useMemo(
    () => aggregateEdges(messages, handles, windowMs, now),
    [messages, handles, windowMs, now],
  );
  const cards = useMemo(() => activeCards(board), [board]);

  const channelSlugs = useMemo(() => {
    const seen = new Set(channels.map((c) => `#${c.channel.slug}`));
    for (const e of edges) {
      if (e.to.startsWith('#')) seen.add(e.to);
      if (e.from.startsWith('#')) seen.add(e.from);
    }
    return [...seen].sort();
  }, [channels, edges]);

  const ids = useMemo(
    () => [...agents.map((a) => `@${a.handle}`), ...channelSlugs],
    [agents, channelSlugs],
  );
  const layout = useMemo(
    () => mergePositions(autoLayout(ids, edges), { ...positions, ...dragging }),
    [ids, edges, positions, dragging],
  );

  const nodes: Node[] = useMemo(() => {
    const out: Node[] = [];
    for (const agent of agents) {
      const id = `@${agent.handle}`;
      const p = layout.get(id) ?? { x: 0, y: 0 };
      const card = cards.get(agent.id) ?? null;
      out.push({
        id,
        type: 'agent',
        position: p,
        data: {
          agent,
          state: stateOf(agent.id),
          preview: lastSaid(messages, agent.handle)?.body ?? null,
          card,
        } satisfies AgentData,
      });
      if (card) {
        out.push({
          id: `card:${card.id}`,
          type: 'card',
          position: { x: p.x + 225, y: p.y + 10 },
          draggable: false,
          data: { card } satisfies CardData,
        });
      }
    }
    for (const slug of channelSlugs) {
      const info = channels.find((c) => `#${c.channel.slug}` === slug);
      out.push({
        id: slug,
        type: 'channel',
        position: layout.get(slug) ?? { x: 0, y: 0 },
        data: {
          slug: slug.slice(1),
          count: messages.filter((m) => m.to === slug).length,
          members: info?.members.length ?? 0,
        } satisfies ChannelData,
      });
    }
    return out;
  }, [agents, channelSlugs, channels, layout, cards, messages, stateOf]);

  const flowEdges: Edge[] = useMemo(() => {
    const out: Edge[] = edges.map((e) => {
      const pending = e.askRemaining !== null;
      return {
        id: e.id,
        source: e.from,
        target: e.to,
        animated: now - e.last < LIVE_MS,
        label: pending
          ? `pergunta · ${formatCountdown(e.askRemaining ?? 0)}`
          : e.count > 1
            ? `${e.count}`
            : undefined,
        data: { preview: e.preview },
        style: {
          strokeWidth: strokeWidth(e.count),
          stroke: e.failed ? 'var(--state-failed)' : 'var(--border-strong)',
          strokeDasharray: pending ? '6 4' : undefined,
        },
      };
    });
    for (const agent of agents) {
      const card = cards.get(agent.id);
      if (!card) continue;
      out.push({
        id: `card-edge:${card.id}`,
        source: `@${agent.handle}`,
        target: `card:${card.id}`,
        style: { strokeDasharray: '2 4', stroke: 'var(--fg-muted)' },
      });
    }
    return out;
  }, [edges, agents, cards, now]);

  return (
    <div className="relative flex h-full min-h-0 flex-col">
      <div className="flex items-center gap-2 border-b border-subtle px-3 py-1.5 text-caption">
        <Button size="sm" variant="ghost" onClick={() => onPositions({})}>
          <LayoutDashboard size={12} /> Auto-organizar
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => setFrozen((f) => !f)}
          aria-pressed={frozen}
        >
          {frozen ? <Play size={12} /> : <Pause size={12} />} {frozen ? 'Retomar' : 'Congelar'}
        </Button>
        <label className="ml-auto flex items-center gap-1 text-muted">
          Últimas
          <select
            value={String(windowMs)}
            onChange={(e) => setWindowMs(Number(e.target.value))}
            className="rounded-sm border border-subtle bg-surface px-1 py-0.5 text-primary"
          >
            {FLOW_WINDOWS.map((w) => (
              <option key={w.label} value={String(w.ms)}>
                {w.label}
              </option>
            ))}
          </select>
        </label>
      </div>
      <section className="min-h-0 flex-1" aria-label="Fluxo da equipe">
        <ReactFlow
          nodes={nodes}
          edges={flowEdges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          fitView
          minZoom={0.3}
          proOptions={{ hideAttribution: true }}
          onNodeDrag={(_, node) => setDragging((d) => ({ ...d, [node.id]: node.position }))}
          onNodeDragStop={(_, node) => {
            setDragging({});
            onPositions({ ...positions, [node.id]: node.position });
          }}
          onNodeClick={(_, node) => {
            if (node.type === 'card') setOpenCard((node.data as CardData).card.id);
          }}
          onNodeDoubleClick={(_, node) => {
            if (node.type === 'agent') onOpenAgent((node.data as AgentData).agent.id);
          }}
        >
          <Background gap={24} size={1} />
        </ReactFlow>
      </section>
      {openCard && board && (
        <CardDetailPanel
          teamId={teamId}
          cardId={openCard}
          columns={board.columns}
          agents={board.agents}
          version={board.cards.find((c) => c.id === openCard)?.version ?? 0}
          onOpenCard={setOpenCard}
          onClose={() => setOpenCard(null)}
        />
      )}
    </div>
  );
}
