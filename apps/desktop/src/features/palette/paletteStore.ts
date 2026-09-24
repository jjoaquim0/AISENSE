import { useEffect } from 'react';
import { create } from 'zustand';

/** Uma ação da paleta (T10). `id` estável: é por ele que "recentes" lembra. */
export interface PaletteAction {
  id: string;
  label: string;
  group: string;
  /** Palavras extras para a busca difusa (ex.: "iniciar ligar play"). */
  keywords?: string[];
  /** Atalho mostrado à direita (`⌘G`). */
  shortcut?: string;
  run: () => unknown;
}

interface PaletteState {
  open: boolean;
  setOpen: (open: boolean) => void;
  /** Ações por origem (a tela aberta registra as suas; sai, somem). */
  sources: Record<string, PaletteAction[]>;
  /** `> enviar @alguem texto` — quem sabe enviar (a Sala da Equipe) registra. */
  send: ((to: string, body: string) => Promise<unknown>) | null;
  register: (source: string, actions: PaletteAction[]) => void;
  unregister: (source: string) => void;
  setSend: (send: PaletteState['send']) => void;
}

export const usePalette = create<PaletteState>((set) => ({
  open: false,
  setOpen: (open) => set({ open }),
  sources: {},
  send: null,
  register: (source, actions) => set((s) => ({ sources: { ...s.sources, [source]: actions } })),
  unregister: (source) =>
    set((s) => {
      const { [source]: _, ...rest } = s.sources;
      return { sources: rest };
    }),
  setSend: (send) => set({ send }),
}));

/** A tela registra suas ações enquanto está aberta. */
export function usePaletteActions(source: string, actions: PaletteAction[]): void {
  const register = usePalette((s) => s.register);
  const unregister = usePalette((s) => s.unregister);
  useEffect(() => {
    register(source, actions);
    return () => unregister(source);
  }, [source, actions, register, unregister]);
}
