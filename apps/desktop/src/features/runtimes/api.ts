import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { RuntimeOverview } from '@/types/generated/RuntimeOverview';

/** Único ponto do front que chama os comandos de runtimes (docs/10). */
export const runtimesApi = {
  /** `refresh` ignora o cache e roda o `detect` de todos de novo. */
  overview: (refresh = false): Promise<RuntimeOverview> =>
    invoke<RuntimeOverview>('runtimes_overview', { refresh }),
};

/** Um adaptador foi salvo, criado ou apagado em disco: a lista deve ser pedida de novo. */
export function onAdaptersChanged(handler: () => void): Promise<UnlistenFn> {
  return listen('adapters:changed', handler);
}
