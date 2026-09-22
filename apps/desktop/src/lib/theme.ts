import { create } from 'zustand';

export type Theme = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'aisense.theme';

function readStoredTheme(): Theme {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved === 'light' || saved === 'dark' ? saved : 'system';
  } catch {
    // localStorage pode lançar em modo privado ou com cookies bloqueados.
    return 'system';
  }
}

function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Preferência não persistida; a sessão atual continua correta.
  }
}

/** `true` quando o tema efetivo (resolvendo `system`) é o escuro. */
export function isDark(theme: Theme): boolean {
  if (theme !== 'system') return theme === 'dark';
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

interface ThemeStore {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  /** Alterna entre claro e escuro a partir do tema efetivo atual. */
  toggle: () => void;
}

export const useTheme = create<ThemeStore>((set, get) => ({
  theme: readStoredTheme(),
  setTheme: (theme) => {
    applyTheme(theme);
    set({ theme });
  },
  toggle: () => get().setTheme(isDark(get().theme) ? 'light' : 'dark'),
}));
