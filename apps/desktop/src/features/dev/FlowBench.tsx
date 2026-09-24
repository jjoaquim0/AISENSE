import '@xyflow/react/dist/style.css';
import { Background, type Edge, type Node, ReactFlow } from '@xyflow/react';
import { useEffect, useMemo, useState } from 'react';
import { nodeTypes } from '@/features/flow/FlowView';
import { aggregateEdges, autoLayout, LIVE_MS, strokeWidth } from '@/features/flow/flowModel';
import type { Agent } from '@/types/generated/Agent';
import type { AgentColor } from '@/types/generated/AgentColor';
import type { MessageView } from '@/types/generated/MessageView';
import { useFps } from './GridBench';

const HANDLES = ['coordenador', 'backend', 'frontend', 'revisor', 'docs', 'qa'];
const IDS = [...HANDLES.map((h) => `@${h}`), '#geral'];
const COLORS: AgentColor[] = ['violet', 'indigo', 'emerald', 'amber', 'rose', 'cyan'];

/**
 * Banco da vista Fluxo (F07-03, `#/dev/flow`): 6 agentes trocando uma mensagem a cada 150 ms,
 * com os mesmos nós e o mesmo modelo de arestas da tela real. O aceite é ≥50 fps.
 */
export function FlowBench() {
  const fps = useFps();
  const [messages, setMessages] = useState<MessageView[]>([]);
  const [now, setNow] = useState(() => Date.now());
  const agents = useMemo(
    () =>
      HANDLES.map((handle, i) => ({ id: `a${i}`, handle, color: COLORS[i] ?? 'violet' }) as Agent),
    [],
  );

  useEffect(() => {
    let n = 0;
    const timer = setInterval(() => {
      n += 1;
      const from = HANDLES[n % HANDLES.length] ?? 'backend';
      const to = HANDLES[(n * 7 + 1) % HANDLES.length] ?? 'frontend';
      const t = Date.now();
      setNow(t);
      if (from === to) return;
      setMessages((list) =>
        [
          ...list,
          {
            id: `m${n}`,
            kind: 'message',
            from: `@${from}`,
            to: n % 5 === 0 ? '#geral' : `@${to}`,
            subject: null,
            body: `mensagem ${n}`,
            replyTo: null,
            meta: { priority: 'normal', attachments: [] },
            createdAt: t,
            receipts: { recipients: 1, delivered: 1, read: 0, failed: 0 },
          } satisfies MessageView,
        ].slice(-2000),
      );
    }, 150);
    return () => clearInterval(timer);
  }, []);

  const edges = useMemo(() => aggregateEdges(messages, HANDLES, 300_000, now), [messages, now]);
  const layout = useMemo(() => autoLayout(IDS, edges), [edges]);
  const nodes: Node[] = [
    ...agents.map((agent) => ({
      id: `@${agent.handle}`,
      type: 'agent',
      position: layout.get(`@${agent.handle}`) ?? { x: 0, y: 0 },
      data: { agent, state: 'busy', preview: `@${agent.handle} trabalhando`, card: null },
    })),
    {
      id: '#geral',
      type: 'channel',
      position: layout.get('#geral') ?? { x: 0, y: 0 },
      data: { slug: 'geral', count: messages.length, members: 0 },
    },
  ];
  const flowEdges: Edge[] = edges.map((e) => ({
    id: e.id,
    source: e.from,
    target: e.to,
    animated: now - e.last < LIVE_MS,
    label: `${e.count}`,
    style: { strokeWidth: strokeWidth(e.count), stroke: 'var(--border-strong)' },
  }));

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-3 border-b border-subtle px-3 py-2 text-label">
        Fluxo: 6 agentes, 1 mensagem a cada 150 ms · {messages.length} mensagens
        <span className="ml-auto font-mono text-primary tabular-nums" data-testid="fps">
          {fps} fps
        </span>
      </div>
      <div className="min-h-0 flex-1">
        <ReactFlow
          nodes={nodes}
          edges={flowEdges}
          nodeTypes={nodeTypes}
          fitView
          proOptions={{ hideAttribution: true }}
        >
          <Background gap={24} size={1} />
        </ReactFlow>
      </div>
    </div>
  );
}
