import type { MessageView } from '@/types/generated/MessageView';

/** Regras da Linha do tempo (T4.4, docs/09), sem React. */

/** Junta mensagens novas às que já estão na tela: sem repetir, mais antiga primeiro. A
 * versão que chega por último vence (recibos atualizados). */
export function mergeMessages(current: MessageView[], incoming: MessageView[]): MessageView[] {
  const byId = new Map(current.map((m) => [m.id, m]));
  for (const m of incoming) byId.set(m.id, m);
  // ULID monotônico: a ordem do id é a ordem de criação.
  return [...byId.values()].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
}

/** Segundos que faltam para um `ask` sem resposta expirar; `null` se não for pergunta
 * pendente (respondida, expirada ou não é pergunta). */
export function askRemaining(message: MessageView, all: MessageView[], now: number): number | null {
  if (message.kind !== 'request') return null;
  if (all.some((m) => m.replyTo === message.id)) return null;
  const timeout = message.meta.timeoutS ?? 300;
  const left = Math.ceil((message.createdAt + timeout * 1000 - now) / 1000);
  return left > 0 ? left : null;
}

export interface TimelineFilter {
  /** `@handle`: mensagens de ou para ele. */
  agent: string | null;
  /** Esconde registros e avisos de sistema. */
  onlyConversation: boolean;
}

export function applyFilter(messages: MessageView[], filter: TimelineFilter): MessageView[] {
  return messages.filter((m) => {
    if (filter.onlyConversation && (m.kind === 'system' || m.kind === 'event')) return false;
    if (filter.agent && m.from !== filter.agent && m.to !== filter.agent) return false;
    return true;
  });
}

/** Perto do fim da lista? Só então mensagem nova rola a tela sozinha. */
export function nearBottom(el: { scrollTop: number; scrollHeight: number; clientHeight: number }) {
  return el.scrollHeight - el.scrollTop - el.clientHeight < 48;
}

/** "✓ entregue", "✓✓ lida por 2 de 3". */
export function receiptLabel(message: MessageView): string | null {
  const { recipients, delivered, read } = message.receipts;
  if (recipients === 0) return null;
  if (read === recipients) return recipients === 1 ? 'lida' : `lida por todos (${recipients})`;
  if (read > 0) return `lida por ${read} de ${recipients}`;
  if (delivered > 0)
    return recipients === 1 ? 'entregue' : `entregue a ${delivered} de ${recipients}`;
  return recipients === 1 ? 'na caixa' : `na caixa de ${recipients}`;
}

/** Destinos do compositor: a equipe, cada agente e os canais já usados. */
export function destinations(handles: string[], messages: MessageView[]): string[] {
  const channels = new Set(
    messages.flatMap((m) => [m.to, m.from]).filter((a) => a.startsWith('#')),
  );
  return ['@all', ...handles.map((h) => `@${h}`), ...[...channels].sort()];
}

export function formatCountdown(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return m > 0 ? `${m}:${String(s).padStart(2, '0')}` : `${s}s`;
}
