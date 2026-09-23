import { create } from 'zustand';

/** Telas de primeiro nível, trocadas pelo trilho esquerdo (docs/09). */
export type Screen = 'teams' | 'skills';

interface NavState {
  screen: Screen;
  go: (screen: Screen) => void;
}

export const useNav = create<NavState>((set) => ({
  screen: 'teams',
  go: (screen) => set({ screen }),
}));
