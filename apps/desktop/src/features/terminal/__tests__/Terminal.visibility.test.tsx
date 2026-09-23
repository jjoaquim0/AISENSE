import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// O xterm precisa de canvas; aqui só importa o contrato com o core.
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    rows = 24;
    cols = 80;
    options = {};
    open() {}
    write() {}
    reset() {}
    clear() {}
    dispose() {}
    loadAddon() {}
    attachCustomKeyEventHandler() {}
    onData() {
      return { dispose() {} };
    }
  },
}));
vi.mock('@xterm/xterm/css/xterm.css', () => ({}));
vi.mock('@xterm/addon-fit', () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock('@xterm/addon-search', () => ({ SearchAddon: class {} }));
vi.mock('@xterm/addon-web-links', () => ({ WebLinksAddon: class {} }));
vi.mock('@xterm/addon-webgl', () => ({ WebglAddon: class {} }));
vi.mock('@/lib/theme', () => ({ useTheme: () => ({ theme: 'dark' }) }));
vi.mock('../theme', () => ({ readTerminalTheme: () => ({}) }));
vi.mock('@/lib/events', () => ({ onPtyData: () => () => {}, onPtyExit: () => () => {} }));

const api = vi.hoisted(() => ({
  show: vi.fn(),
  setVisible: vi.fn(),
  resize: vi.fn(),
  write: vi.fn(),
  clear: vi.fn(),
}));
vi.mock('../api', () => ({ terminalApi: api }));

const { Terminal } = await import('../Terminal');
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('visibilidade do terminal (F03-05)', () => {
  let host: HTMLDivElement;
  beforeEach(() => {
    host = document.createElement('div');
    document.body.append(host);
    for (const fn of Object.values(api)) fn.mockReset().mockResolvedValue(undefined);
    api.show.mockResolvedValue('');
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        disconnect() {}
      },
    );
  });
  afterEach(() => {
    host.remove();
    vi.unstubAllGlobals();
  });

  it('ao aparecer liga os eventos; ao ser escondido ou desmontado, desliga', async () => {
    const root = createRoot(host);
    await act(async () => root.render(<Terminal agentId="agt_1" />));
    expect(api.show).toHaveBeenCalledWith('agt_1');
    expect(api.setVisible).not.toHaveBeenCalled();

    await act(async () => root.render(<Terminal agentId="agt_1" visible={false} />));
    expect(api.setVisible).toHaveBeenLastCalledWith('agt_1', false);

    await act(async () => root.render(<Terminal agentId="agt_1" />));
    expect(api.show).toHaveBeenCalledTimes(2);

    await act(async () => root.unmount());
    expect(api.setVisible).toHaveBeenLastCalledWith('agt_1', false);
  });

  it('montado escondido não liga os eventos', async () => {
    const root = createRoot(host);
    await act(async () => root.render(<Terminal agentId="agt_2" visible={false} />));
    expect(api.show).not.toHaveBeenCalled();
    await act(async () => root.unmount());
  });

  it('com a janela em segundo plano, desliga; ao voltar, liga de novo', async () => {
    const root = createRoot(host);
    await act(async () => root.render(<Terminal agentId="agt_3" />));
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    await act(async () => document.dispatchEvent(new Event('visibilitychange')));
    expect(api.setVisible).toHaveBeenLastCalledWith('agt_3', false);
    hidden.mockReturnValue(false);
    await act(async () => document.dispatchEvent(new Event('visibilitychange')));
    expect(api.show).toHaveBeenCalledTimes(2);
    await act(async () => root.unmount());
  });
});
