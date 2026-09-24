import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import { CommandPalette } from '../CommandPalette';
import { grouped, parseSend, pushRecent, recentActions } from '../paletteModel';
import { type PaletteAction, usePalette } from '../paletteStore';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
// jsdom não rola nem mede; o cmdk só precisa que existam.
Element.prototype.scrollIntoView = () => {};
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

const action = (id: string, label: string, group = 'Vista', run = vi.fn()): PaletteAction => ({
  id,
  label,
  group,
  run,
});

describe('modelo da paleta', () => {
  it('lê envio, recentes e grupos', () => {
    expect(parseSend('> enviar @backend subiu o contrato')).toEqual({
      to: '@backend',
      body: 'subiu o contrato',
    });
    expect(parseSend('>#geral bom dia')).toEqual({ to: '#geral', body: 'bom dia' });
    expect(parseSend('> enviar @backend')).toBeNull();
    expect(parseSend('vista fluxo')).toBeNull();
    const recent = pushRecent('b', pushRecent('a', []));
    expect(recent).toEqual(['b', 'a']);
    expect(pushRecent('a', recent)).toEqual(['a', 'b']);
    const list = [action('a', 'A'), action('b', 'B', 'Equipe')];
    expect(recentActions(list, ['b', 'sumiu', 'a']).map((a) => a.id)).toEqual(['b', 'a']);
    expect(grouped(list).map(([g]) => g)).toEqual(['Vista', 'Equipe']);
  });
});

describe('<CommandPalette />', () => {
  const type = async (input: HTMLInputElement, value: string) => {
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(input, value);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
  };
  const enter = async (input: HTMLInputElement) => {
    await act(async () => {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
  };

  it('busca difusa, Enter executa sem mouse e > envia mensagem', async () => {
    const fluxo = vi.fn();
    const send = vi.fn().mockResolvedValue(['m1']);
    usePalette.setState({
      open: true,
      send,
      sources: {
        sala: [
          action('view:flow', 'Vista Fluxo', 'Vista', fluxo),
          action('view:grid', 'Vista Grade'),
        ],
      },
    });
    const root = createRoot(document.createElement('div'));
    await act(async () =>
      root.render(
        <CommandPalette global={[action('go:skills', 'Biblioteca de skills', 'Ir para')]} />,
      ),
    );
    const input = document.querySelector('[cmdk-input]') as HTMLInputElement;
    expect(document.body.textContent).toContain('Biblioteca de skills');
    await type(input, 'fluxo');
    const items = [...document.querySelectorAll('[cmdk-item]')].map((i) => i.textContent);
    expect(items).toEqual(['Vista Fluxo']);
    await enter(input);
    expect(fluxo).toHaveBeenCalled();
    expect(usePalette.getState().open).toBe(false);

    // Reaberta: o usado por último aparece em Recentes, no topo.
    await act(async () => usePalette.getState().setOpen(true));
    const headings = [...document.querySelectorAll('[cmdk-group-heading]')].map(
      (h) => h.textContent,
    );
    expect(headings[0]).toBe('Recentes');

    const input2 = document.querySelector('[cmdk-input]') as HTMLInputElement;
    await type(input2, '> enviar @backend subiu o contrato');
    expect(document.body.textContent).toContain('Enviar para @backend: subiu o contrato');
    await enter(input2);
    expect(send).toHaveBeenCalledWith('@backend', 'subiu o contrato');
    act(() => root.unmount());
  });
});
