import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { UpdateInfo } from '@/types/generated/UpdateInfo';
import type { UpdateProgress } from '@/types/generated/UpdateProgress';

/** Único ponto do front que chama o updater (F09-03). A verificação da assinatura é do core. */
export const updatesApi = {
  /** `null` quando já está na versão mais nova. */
  check: (): Promise<UpdateInfo | null> => invoke('update_check'),
  /** Baixa, confere a assinatura, instala e reinicia o app. Só volta se falhar. */
  install: (): Promise<void> => invoke('update_install'),
};

export function onUpdateProgress(handler: (p: UpdateProgress) => void): Promise<UnlistenFn> {
  return listen<UpdateProgress>('update:progress', ({ payload }) => handler(payload));
}
