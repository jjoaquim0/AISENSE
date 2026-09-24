import { act, useMemo } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { applies, type Bindings, comboOf } from '../shortcuts';
import { useShortcuts } from '../useShortcuts';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const key = (init: KeyboardEventInit & { key: string }) =>
  new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });

describe('comboOf', () => {
  it('usa o dígito físico, para ⌘1 ser a mesma tecla em qualquer layout', () => {
    // AZERTY: a tecla do 1 sem Shift produz "&".
    expect(comboOf(key({ key: '&', code: 'Digit1', ctrlKey: true }))).toBe('⌘1');
    expect(comboOf(key({ key: 'd', code: 'KeyD', metaKey: true, shiftKey: true }))).toBe('⌘⇧D');
    expect(comboOf(key({ key: '\\', code: 'Backslash', ctrlKey: true }))).toBe('⌘\\');
    expect(comboOf(key({ key: 'Escape', code: 'Escape' }))).toBe('Escape');
    expect(comboOf(key({ key: 'g', code: 'KeyG', ctrlKey: true, altKey: true }))).toBeNull();
  });
});

describe('applies', () => {
  const run = () => {};
  it('fora do terminal, tudo vale', () => {
    expect(applies({ run }, false, false)).toBe(true);
  });
  it('no terminal, fora do macOS, só o que não é tecla de shell', () => {
    expect(applies({ run }, true, false)).toBe(false);
    expect(applies({ run, inTerminal: 'always' }, true, false)).toBe(true);
    expect(applies({ run }, true, true)).toBe(true);
  });
});

/** Um "xterm": textarea escondido dentro de `.xterm`, que manda ao shell o que ouve. */
function fakeTerminal() {
  const pane = document.createElement('div');
  pane.dataset.agentPane = 'a';
  pane.tabIndex = -1;
  const xterm = document.createElement('div');
  xterm.className = 'xterm';
  const textarea = document.createElement('textarea');
  xterm.append(textarea);
  pane.append(xterm);
  document.body.append(pane);
  const shell = vi.fn();
  textarea.addEventListener('keydown', (e) => shell(e.key));
  textarea.focus();
  return { pane, textarea, shell };
}

function Layer({ bindings }: { bindings: Bindings }) {
  const stable = useMemo(() => bindings, [bindings]);
  useShortcuts(stable);
  return null;
}

describe('camada de atalhos', () => {
  let host: HTMLDivElement;
  let root: Root;
  beforeEach(() => {
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
  });
  afterEach(() => {
    act(() => root.unmount());
    document.body.innerHTML = '';
  });

  const mount = (bindings: Bindings) => act(() => root.render(<Layer bindings={bindings} />));

  it('com o terminal focado, ⌘1 funciona e não vaza a tecla para o shell', () => {
    const focus = vi.fn();
    mount({ '⌘1': { run: focus, inTerminal: 'always' } });
    const { textarea, shell } = fakeTerminal();

    const event = key({ key: '1', code: 'Digit1', ctrlKey: true });
    textarea.dispatchEvent(event);

    expect(focus).toHaveBeenCalledTimes(1);
    expect(shell).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });

  it('fora do macOS, Ctrl+W no terminal continua sendo do shell (apagar palavra)', () => {
    const close = vi.fn();
    mount({ '⌘W': close });
    const { textarea, shell } = fakeTerminal();

    textarea.dispatchEvent(key({ key: 'w', code: 'KeyW', ctrlKey: true }));

    expect(close).not.toHaveBeenCalled();
    expect(shell).toHaveBeenCalledWith('w');
  });

  it('fora do terminal, ⌘W vale', () => {
    const close = vi.fn();
    mount({ '⌘W': close });
    host.dispatchEvent(key({ key: 'w', code: 'KeyW', ctrlKey: true }));
    expect(close).toHaveBeenCalledTimes(1);
  });

  it('Esc Esc sai do terminal para o painel; a primeira Esc ainda chega ao processo', () => {
    mount({});
    const { pane, textarea, shell } = fakeTerminal();

    textarea.dispatchEvent(key({ key: 'Escape', code: 'Escape' }));
    expect(shell).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(textarea);

    textarea.dispatchEvent(key({ key: 'Escape', code: 'Escape' }));
    expect(shell).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(pane);
  });

  it('não dispara dentro de um diálogo', () => {
    const newAgent = vi.fn();
    mount({ '⌘T': newAgent });
    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    const input = document.createElement('input');
    dialog.append(input);
    document.body.append(dialog);
    input.dispatchEvent(key({ key: 't', code: 'KeyT', ctrlKey: true }));
    expect(newAgent).not.toHaveBeenCalled();
  });

  it('no conflito vale a camada mais recente, e ao desmontar ela sai', () => {
    const shell = vi.fn();
    const room = vi.fn();
    const other = createRoot(document.createElement('div'));
    mount({ '⌘G': shell });
    act(() => other.render(<Layer bindings={{ '⌘G': room }} />));
    document.body.dispatchEvent(key({ key: 'g', code: 'KeyG', ctrlKey: true }));
    expect(room).toHaveBeenCalledTimes(1);
    expect(shell).not.toHaveBeenCalled();
    act(() => other.unmount());
    document.body.dispatchEvent(key({ key: 'g', code: 'KeyG', ctrlKey: true }));
    expect(shell).toHaveBeenCalledTimes(1);
  });
});

describe('remapeamento (F08-05)', () => {
  it('a tecla escolhida aciona o atalho padrão e a antiga deixa de valer', async () => {
    const { buildRemap, resolveCombo } = await import('../shortcuts');
    const remap = buildRemap({ '⌘B': '⌘⇧B', '⌘I': '⌘I' });
    expect(resolveCombo('⌘⇧B', remap)).toBe('⌘B');
    expect(resolveCombo('⌘B', remap)).toBeNull();
    expect(resolveCombo('⌘I', remap)).toBe('⌘I');
    expect(resolveCombo('⌘K', remap)).toBe('⌘K');
  });

  it('gravar só aceita combinações com ⌘', async () => {
    const { recordCombo } = await import('../shortcuts');
    expect(recordCombo(key({ key: 'Control', code: 'ControlLeft', ctrlKey: true }))).toBeNull();
    expect(recordCombo(key({ key: 'x', code: 'KeyX' }))).toBeNull();
    expect(recordCombo(key({ key: 'y', code: 'KeyY', ctrlKey: true, shiftKey: true }))).toBe('⌘⇧Y');
  });
});
