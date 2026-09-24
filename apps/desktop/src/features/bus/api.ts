import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentId } from '@/types/generated/AgentId';
import type { BusMessageEvent } from '@/types/generated/BusMessageEvent';
import type { MessageId } from '@/types/generated/MessageId';
import type { MessageView } from '@/types/generated/MessageView';
import type { TeamId } from '@/types/generated/TeamId';
import type { UnreadCount } from '@/types/generated/UnreadCount';

/** Único ponto do front que chama os comandos do barramento (docs/07). */
export const busApi = {
  /** Mais nova primeiro, antes do cursor. */
  timeline: (teamId: TeamId, before?: MessageId, limit = 100): Promise<MessageView[]> =>
    invoke('bus_timeline', { teamId, before: before ?? null, limit }),
  /** Você (`@voce`) manda: `@agente`, `#canal` ou `@all`. */
  send: (teamId: TeamId, to: string[], body: string): Promise<MessageId[]> =>
    invoke('bus_send', { teamId, to, body }),
  unread: (teamId: TeamId): Promise<UnreadCount[]> => invoke('bus_unread', { teamId }),
};

/** Toda mensagem roteada, de qualquer equipe, já com os nomes. */
export function onBusMessage(handler: (event: BusMessageEvent) => void): Promise<UnlistenFn> {
  return listen<BusMessageEvent>('bus:message', ({ payload }) => handler(payload));
}

/** Um agente leu mensagens (o badge dele muda). */
export function onBusRead(handler: (agentId: AgentId) => void): Promise<UnlistenFn> {
  return listen<AgentId>('bus:read', ({ payload }) => handler(payload));
}
