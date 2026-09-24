import { create } from 'zustand';
import { errorMessage } from '@/features/teams/api';
import { buildRemap } from '@/lib/shortcuts';
import { useTheme } from '@/lib/theme';
import { setShortcutRemap } from '@/lib/useShortcuts';
import type { AppSettings } from '@/types/generated/AppSettings';
import type { SettingsView } from '@/types/generated/SettingsView';
import { onSettingsChanged, settingsApi } from './api';

interface SettingsState {
  /** `null` até a primeira carga. */
  view: SettingsView | null;
  error: string | null;
  load: () => Promise<void>;
  /** Aplica `change` a uma cópia e grava. O formulário inteiro passa por aqui. */
  update: (change: (draft: AppSettings) => void) => Promise<void>;
  /** Chamado por quem gravou por outro comando (segredos, reset, onboarding). */
  received: (settings: AppSettings) => void;
}

/** O que muda fora do formulário quando as preferências mudam. */
export function applySettings(settings: AppSettings): void {
  const theme = useTheme.getState();
  if (theme.theme !== settings.appearance.theme) theme.setTheme(settings.appearance.theme);
  setShortcutRemap(buildRemap(settings.shortcuts as Record<string, string>));
  document.documentElement.dataset.density = settings.appearance.density;
}

/** O que o xterm recebe das preferências (fonte, tamanho e densidade). */
export interface TerminalFont {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
}

export function terminalFontOf(settings: AppSettings | null | undefined): TerminalFont {
  const a = settings?.appearance;
  const family = a?.terminalFontFamily.trim();
  return {
    // A do design system continua como reserva: fonte digitada errada não quebra a tela.
    fontFamily: family ? `"${family.replace(/"/g, '')}", var(--font-mono)` : 'var(--font-mono)',
    fontSize: a?.terminalFontSize ?? 13,
    lineHeight: a?.density === 'compact' ? 1.2 : 1.4,
  };
}

let listening = false;

export const useSettings = create<SettingsState>((set, get) => ({
  view: null,
  error: null,
  load: async () => {
    try {
      const view = await settingsApi.get();
      set({ view, error: null });
      applySettings(view.settings);
    } catch (e: unknown) {
      set({ error: errorMessage(e) });
    }
    if (!listening) {
      listening = true;
      void onSettingsChanged((settings) => get().received(settings));
      // ⌘⇧D e o botão da barra mudam o tema direto; a preferência acompanha.
      useTheme.subscribe(({ theme }) => {
        const current = get().view?.settings.appearance.theme;
        if (current && current !== theme) {
          void get().update((d) => {
            d.appearance.theme = theme;
          });
        }
      });
    }
  },
  update: async (change) => {
    const view = get().view;
    if (!view) return;
    const draft = structuredClone(view.settings);
    change(draft);
    // Otimista: o campo não pisca enquanto o core grava.
    set({ view: { ...view, settings: draft } });
    applySettings(draft);
    try {
      get().received(await settingsApi.save(draft));
      set({ error: null });
    } catch (e: unknown) {
      set({ view, error: errorMessage(e) });
      applySettings(view.settings);
    }
  },
  received: (settings) => {
    const view = get().view;
    if (view) set({ view: { ...view, settings } });
    applySettings(settings);
  },
}));
