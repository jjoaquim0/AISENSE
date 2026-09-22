import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { PtyData } from '@/types/generated/PtyData';
import type { PtyExit } from '@/types/generated/PtyExit';

/**
 * Assinatura central dos eventos do core.
 *
 * Um `listen()` por componente vaza handler e vira bug de memória com 9 terminais
 * abertos (docs/10-padroes-de-codigo.md). Aqui há **um** listener por tipo de
 * evento, e os componentes se registram num mapa por agente.
 */
type Handler<T> = (payload: T) => void;

class EventBridge<T extends { agentId: string }> {
  private handlers = new Map<string, Set<Handler<T>>>();
  private unlisten: Promise<UnlistenFn> | null = null;

  constructor(private readonly event: string) {}

  subscribe(agentId: string, handler: Handler<T>): () => void {
    this.ensureListening();

    const existing = this.handlers.get(agentId) ?? new Set<Handler<T>>();
    existing.add(handler);
    this.handlers.set(agentId, existing);

    return () => {
      const handlers = this.handlers.get(agentId);
      if (!handlers) return;
      handlers.delete(handler);
      if (handlers.size === 0) this.handlers.delete(agentId);
    };
  }

  private ensureListening(): void {
    if (this.unlisten) return;
    this.unlisten = listen<T>(this.event, ({ payload }) => {
      for (const handler of this.handlers.get(payload.agentId) ?? []) {
        handler(payload);
      }
    });
  }
}

const dataBridge = new EventBridge<PtyData>('pty:data');
const exitBridge = new EventBridge<PtyExit>('pty:exit');

export function onPtyData(agentId: string, handler: Handler<PtyData>): () => void {
  return dataBridge.subscribe(agentId, handler);
}

export function onPtyExit(agentId: string, handler: Handler<PtyExit>): () => void {
  return exitBridge.subscribe(agentId, handler);
}
