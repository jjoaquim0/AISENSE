import { invoke } from '@tauri-apps/api/core';
import type { AppInfo } from '@/types/generated/AppInfo';

/**
 * Único ponto do front que fala com o core em Rust (AGENTS.md, docs/10).
 * Componentes nunca chamam `invoke` diretamente.
 */
export const api = {
  appInfo: (): Promise<AppInfo> => invoke<AppInfo>('app_info'),
};

/** `true` quando estamos dentro da janela do Tauri (e não no `vite dev` puro). */
export function isDesktop(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
