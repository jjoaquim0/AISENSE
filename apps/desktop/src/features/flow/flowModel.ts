import { askRemaining } from '@/features/timeline/timelineModel';
import type { MessageView } from '@/types/generated/MessageView';

/** Janela de tempo do canvas (docs/09, T4.3: "últimas: 5min ▾"). */
export const FLOW_WINDOWS = [
  { label: '5 min', ms: 5 * 60_000 },
  { label: '30 min', ms: 30 * 60_000 },
  { label: '2 h', ms: 2 * 3_600_000 },
  { label: 'tudo', ms: Number.POSITIVE_INFINITY },
] as const;

/** Aresta "viva" (animada) enquanto a última mensagem é recente. */
export const LIVE_MS = 4_000;

export interface FlowEdge {
  id: string;
  /** `@handle` ou `#canal`. */
  from: string;
  to: string;
  count: number;
  last: number;
  /** Última mensagem, para o rótulo. */
  preview: string;
  /** Pergunta sem resposta: segundos até expirar (tracejado com contagem). */
  askRemaining: number | null;
  /** Alguma entrega falhou ou expirou (vermelho). */
  failed: boolean;
}

/** Quem conta como nó: agentes da equipe e canais. `@voce`, `@all` e o sistema não. */
function endpoint(label: string, handles: Set<string>): string | null {
  if (label.startsWith('#')) return label;
  return handles.has(label) ? label : null;
}

/**
 * Uma aresta por par (de → para), com o volume da janela — agregar mantém o canvas leve
 * mesmo com milhares de mensagens (FASE-07, riscos).
 */
export function aggregateEdges(
  messages: MessageView[],
  handles: string[],
  windowMs: number,
  now: number,
): FlowEdge[] {
  const known = new Set(handles.map((h) => `@${h}`));
  const edges = new Map<string, FlowEdge>();
  for (const m of messages) {
    if (now - m.createdAt > windowMs) continue;
    const from = endpoint(m.from, known);
    const to = endpoint(m.to, known);
    if (!from || !to || from === to) continue;
    const id = `${from}->${to}`;
    const edge = edges.get(id) ?? {
      id,
      from,
      to,
      count: 0,
      last: 0,
      preview: '',
      askRemaining: null,
      failed: false,
    };
    edge.count += 1;
    if (m.createdAt >= edge.last) {
      edge.last = m.createdAt;
      edge.preview = m.body.split('\n')[0] ?? '';
    }
    const remaining = askRemaining(m, messages, now);
    if (remaining !== null) {
      edge.askRemaining =
        edge.askRemaining === null ? remaining : Math.min(edge.askRemaining, remaining);
    }
    if (m.receipts.failed > 0) edge.failed = true;
    edges.set(id, edge);
  }
  return [...edges.values()];
}

/** Espessura pelo volume: 1 mensagem = 1.5px, cresce devagar. */
export function strokeWidth(count: number): number {
  return Math.min(6, 1.5 + Math.log2(count));
}

export interface Point {
  x: number;
  y: number;
}

const COL = 260;
const ROW = 150;

/**
 * Layout automático em camadas: quem manda fica acima de quem recebe (como o dagre faria
 * para um grafo pequeno), sem dependência nova. Nós sem aresta vão para a última camada;
 * canais ficam à direita.
 */
export function autoLayout(
  nodes: string[],
  edges: { from: string; to: string }[],
): Map<string, Point> {
  const agents = nodes.filter((n) => !n.startsWith('#'));
  const channels = nodes.filter((n) => n.startsWith('#'));
  const incoming = new Map<string, number>(agents.map((n) => [n, 0]));
  for (const e of edges) {
    if (incoming.has(e.to) && incoming.has(e.from))
      incoming.set(e.to, (incoming.get(e.to) ?? 0) + 1);
  }
  // Camada = distância a partir das fontes (BFS), com ciclo resolvido pela ordem.
  const layer = new Map<string, number>();
  const sources = agents.filter((n) => (incoming.get(n) ?? 0) === 0);
  const queue = sources.length > 0 ? [...sources] : agents.slice(0, 1);
  for (const s of queue) layer.set(s, 0);
  while (queue.length > 0) {
    const current = queue.shift() as string;
    const depth = layer.get(current) ?? 0;
    for (const e of edges) {
      if (e.from === current && incoming.has(e.to) && !layer.has(e.to)) {
        layer.set(e.to, Math.min(depth + 1, 4));
        queue.push(e.to);
      }
    }
  }
  const deepest = Math.max(0, ...layer.values());
  for (const n of agents) if (!layer.has(n)) layer.set(n, deepest + (edges.length > 0 ? 1 : 0));
  const rows = new Map<number, string[]>();
  for (const n of agents) {
    const l = layer.get(n) ?? 0;
    rows.set(l, [...(rows.get(l) ?? []), n]);
  }
  const out = new Map<string, Point>();
  for (const [l, row] of rows) {
    row.forEach((n, i) => {
      out.set(n, { x: i * COL, y: l * ROW });
    });
  }
  const width = Math.max(1, ...[...rows.values()].map((r) => r.length));
  channels.forEach((c, i) => {
    out.set(c, { x: width * COL + 40, y: i * ROW });
  });
  return out;
}

/** Posições salvas (arrastadas à mão) vencem o automático. */
export function mergePositions(
  auto: Map<string, Point>,
  saved: Record<string, Point>,
): Map<string, Point> {
  const out = new Map(auto);
  for (const [id, p] of Object.entries(saved)) if (out.has(id)) out.set(id, p);
  return out;
}

/** Lê `teams.layout.flow.positions`. Qualquer coisa estranha vira vazio. */
export function readFlowPositions(teamLayout: unknown): Record<string, Point> {
  const flow =
    typeof teamLayout === 'object' && teamLayout !== null && 'flow' in teamLayout
      ? (teamLayout as { flow: unknown }).flow
      : undefined;
  const positions =
    typeof flow === 'object' && flow !== null && 'positions' in flow
      ? (flow as { positions: unknown }).positions
      : undefined;
  if (typeof positions !== 'object' || positions === null) return {};
  const out: Record<string, Point> = {};
  for (const [id, p] of Object.entries(positions)) {
    if (
      typeof p === 'object' &&
      p !== null &&
      typeof (p as Point).x === 'number' &&
      typeof (p as Point).y === 'number'
    ) {
      out[id] = { x: (p as Point).x, y: (p as Point).y };
    }
  }
  return out;
}

/** A última coisa que o agente disse (2 linhas no nó). */
export function lastSaid(messages: MessageView[], handle: string): MessageView | null {
  for (let i = messages.length - 1; i >= 0; i--) {
    const m = messages[i];
    if (m && m.from === `@${handle}`) return m;
  }
  return null;
}
