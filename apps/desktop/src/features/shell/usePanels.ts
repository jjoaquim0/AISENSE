import { create } from 'zustand';

const STORAGE_KEY = 'aisense.panels';

export const SIDEBAR_BOUNDS = { min: 200, max: 360, default: 240 } as const;
export const INSPECTOR_BOUNDS = { min: 280, max: 480, default: 320 } as const;

interface PanelState {
  sidebarWidth: number;
  inspectorWidth: number;
  sidebarVisible: boolean;
  inspectorVisible: boolean;
}

const DEFAULTS: PanelState = {
  sidebarWidth: SIDEBAR_BOUNDS.default,
  inspectorWidth: INSPECTOR_BOUNDS.default,
  sidebarVisible: true,
  inspectorVisible: true,
};

export function clamp(value: number, { min, max }: { min: number; max: number }): number {
  return Math.min(max, Math.max(min, value));
}

function load(): PanelState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULTS;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) return DEFAULTS;
    const saved = parsed as Partial<PanelState>;
    // Valores vindos do disco são dados, não verdade: recortam-se aos limites.
    return {
      sidebarWidth: clamp(saved.sidebarWidth ?? DEFAULTS.sidebarWidth, SIDEBAR_BOUNDS),
      inspectorWidth: clamp(saved.inspectorWidth ?? DEFAULTS.inspectorWidth, INSPECTOR_BOUNDS),
      sidebarVisible: saved.sidebarVisible ?? DEFAULTS.sidebarVisible,
      inspectorVisible: saved.inspectorVisible ?? DEFAULTS.inspectorVisible,
    };
  } catch {
    // JSON inválido ou localStorage bloqueado: os padrões servem.
    return DEFAULTS;
  }
}

function persist(state: PanelState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Layout não persistido; a sessão atual continua correta.
  }
}

interface PanelStore extends PanelState {
  setSidebarWidth: (width: number) => void;
  setInspectorWidth: (width: number) => void;
  toggleSidebar: () => void;
  toggleInspector: () => void;
  /** Duplo clique num agente da sidebar abre o inspetor, nunca o fecha. */
  showInspector: () => void;
}

export const usePanels = create<PanelStore>((set, get) => {
  const commit = (patch: Partial<PanelState>): void => {
    const next = { ...pick(get()), ...patch };
    persist(next);
    set(patch);
  };

  return {
    ...load(),
    setSidebarWidth: (width) => commit({ sidebarWidth: clamp(width, SIDEBAR_BOUNDS) }),
    setInspectorWidth: (width) => commit({ inspectorWidth: clamp(width, INSPECTOR_BOUNDS) }),
    toggleSidebar: () => commit({ sidebarVisible: !get().sidebarVisible }),
    toggleInspector: () => commit({ inspectorVisible: !get().inspectorVisible }),
    showInspector: () => {
      if (!get().inspectorVisible) commit({ inspectorVisible: true });
    },
  };
});

function pick(state: PanelStore): PanelState {
  return {
    sidebarWidth: state.sidebarWidth,
    inspectorWidth: state.inspectorWidth,
    sidebarVisible: state.sidebarVisible,
    inspectorVisible: state.inspectorVisible,
  };
}

/** Largura do trilho de equipes (docs/09). */
export const RAIL_WIDTH = 48;
/** Abaixo disto a área principal não serve para um terminal: os painéis cedem (F08-02). */
export const MAIN_MIN = 420;

/**
 * Quais painéis cabem numa janela de `windowWidth` px (CSS — com zoom de 150%, uma
 * janela de 1024 vira 683). Quem cede primeiro é o inspetor, depois a sidebar; a
 * preferência do usuário não muda, e com espaço os dois voltam sozinhos.
 */
export function fitPanels(
  windowWidth: number,
  { sidebarWidth, inspectorWidth, sidebarVisible, inspectorVisible }: PanelState,
): { sidebar: boolean; inspector: boolean } {
  let room = windowWidth - RAIL_WIDTH;
  const sidebar = sidebarVisible && room - sidebarWidth >= MAIN_MIN;
  if (sidebar) room -= sidebarWidth;
  const inspector = inspectorVisible && room - inspectorWidth >= MAIN_MIN;
  return { sidebar, inspector };
}
