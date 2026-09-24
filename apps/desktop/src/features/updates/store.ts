import { create } from 'zustand';
import { errorMessage } from '@/features/teams/api';
import type { UpdateInfo } from '@/types/generated/UpdateInfo';
import { onUpdateProgress, updatesApi } from './api';

export type UpdatePhase =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'current' }
  | { kind: 'available'; update: UpdateInfo }
  | { kind: 'installing'; update: UpdateInfo; percent: number | null }
  | { kind: 'error'; message: string; update: UpdateInfo | null };

interface UpdatesState {
  phase: UpdatePhase;
  /** "Depois" na faixa: some até o app abrir de novo, mas continua nas Configurações. */
  dismissed: boolean;
  check: () => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdates = create<UpdatesState>((set, get) => ({
  phase: { kind: 'idle' },
  dismissed: false,
  check: async () => {
    const { phase } = get();
    if (phase.kind === 'checking' || phase.kind === 'installing') return;
    set({ phase: { kind: 'checking' } });
    try {
      const update = await updatesApi.check();
      set({ phase: update ? { kind: 'available', update } : { kind: 'current' } });
    } catch (e: unknown) {
      set({ phase: { kind: 'error', message: errorMessage(e), update: null } });
    }
  },
  install: async () => {
    const { phase } = get();
    if (phase.kind !== 'available' && !(phase.kind === 'error' && phase.update)) return;
    const update = phase.update as UpdateInfo;
    set({ phase: { kind: 'installing', update, percent: null }, dismissed: false });
    const unlisten = await onUpdateProgress(({ downloaded, total }) => {
      const percent = total ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
      set({ phase: { kind: 'installing', update, percent } });
    });
    try {
      // Sucesso reinicia o app: esta promessa só resolve se algo der errado antes.
      await updatesApi.install();
    } catch (e: unknown) {
      set({ phase: { kind: 'error', message: errorMessage(e), update } });
    } finally {
      unlisten();
    }
  },
  dismiss: () => set({ dismissed: true }),
}));
