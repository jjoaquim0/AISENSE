import { invoke } from '@tauri-apps/api/core';
import type { SpawnRequest } from '@/types/generated/SpawnRequest';

/** Único ponto do front que chama os comandos de terminal (docs/10). */
export const terminalApi = {
  spawn: (request: SpawnRequest): Promise<void> => invoke('pty_spawn', { request }),
  write: (agentId: string, data: string): Promise<void> => invoke('pty_write', { agentId, data }),
  resize: (agentId: string, rows: number, cols: number): Promise<void> =>
    invoke('pty_resize', { agentId, rows, cols }),
  kill: (agentId: string): Promise<void> => invoke('pty_kill', { agentId }),
  /** Histórico em base64, para reidratar o xterm de uma vez só. */
  snapshot: (agentId: string): Promise<string> => invoke('pty_snapshot', { agentId }),
  setVisible: (agentId: string, visible: boolean): Promise<void> =>
    invoke('pty_set_visible', { agentId, visible }),
  isRunning: (agentId: string): Promise<boolean> => invoke('pty_is_running', { agentId }),
};
