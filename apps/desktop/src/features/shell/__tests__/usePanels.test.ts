import { beforeEach, describe, expect, it } from 'vitest';
import { clamp, INSPECTOR_BOUNDS, SIDEBAR_BOUNDS, usePanels } from '../usePanels';

describe('clamp', () => {
  it('mantém valores dentro da faixa', () => {
    expect(clamp(240, SIDEBAR_BOUNDS)).toBe(240);
  });

  it('recorta abaixo do mínimo e acima do máximo', () => {
    expect(clamp(10, SIDEBAR_BOUNDS)).toBe(SIDEBAR_BOUNDS.min);
    expect(clamp(9999, SIDEBAR_BOUNDS)).toBe(SIDEBAR_BOUNDS.max);
  });
});

describe('usePanels', () => {
  beforeEach(() => {
    localStorage.clear();
    usePanels.setState({
      sidebarWidth: SIDEBAR_BOUNDS.default,
      inspectorWidth: INSPECTOR_BOUNDS.default,
      sidebarVisible: true,
      inspectorVisible: true,
    });
  });

  it('nunca deixa um painel sumir ou engolir a tela', () => {
    const { setSidebarWidth, setInspectorWidth } = usePanels.getState();

    setSidebarWidth(-500);
    expect(usePanels.getState().sidebarWidth).toBe(SIDEBAR_BOUNDS.min);

    setInspectorWidth(5000);
    expect(usePanels.getState().inspectorWidth).toBe(INSPECTOR_BOUNDS.max);
  });

  it('persiste o layout para a próxima sessão', () => {
    usePanels.getState().setSidebarWidth(300);
    usePanels.getState().toggleInspector();

    const saved: unknown = JSON.parse(localStorage.getItem('aisense.panels') ?? '{}');
    expect(saved).toMatchObject({ sidebarWidth: 300, inspectorVisible: false });
  });

  it('alternar duas vezes volta ao estado original', () => {
    const before = usePanels.getState().sidebarVisible;
    usePanels.getState().toggleSidebar();
    usePanels.getState().toggleSidebar();
    expect(usePanels.getState().sidebarVisible).toBe(before);
  });
});

describe('fitPanels', () => {
  const prefs = {
    sidebarWidth: 240,
    inspectorWidth: 320,
    sidebarVisible: true,
    inspectorVisible: true,
  };
  it('com espaço, respeita a preferência', async () => {
    const { fitPanels } = await import('../usePanels');
    expect(fitPanels(1440, prefs)).toEqual({ sidebar: true, inspector: true });
    expect(fitPanels(1440, { ...prefs, inspectorVisible: false })).toEqual({
      sidebar: true,
      inspector: false,
    });
  });
  it('sem espaço, o inspetor cede primeiro e depois a sidebar', async () => {
    const { fitPanels } = await import('../usePanels');
    // 1024 px com zoom de 125% ≈ 819 px de CSS.
    expect(fitPanels(819, prefs)).toEqual({ sidebar: true, inspector: false });
    // Com zoom de 150% ≈ 683 px.
    expect(fitPanels(683, prefs)).toEqual({ sidebar: false, inspector: false });
  });
});
