import { useEffect } from 'react';
import {
  applies,
  type Bindings,
  comboOf,
  DOUBLE_ESCAPE_MS,
  isMacPlatform,
  normalize,
} from './shortcuts';

/**
 * Atalhos globais. A chave é `⌘X`, `⌘⇧X` ou apenas `X`; `⌘` casa com Cmd no macOS
 * e Ctrl no resto (docs/08-design-system.md). Regras em `shortcuts.ts`.
 *
 * Um único listener na janela para o app inteiro — `listen()` espalhado por componente
 * vaza handler e vira bug de memória com muitos painéis abertos (docs/10). Cada
 * `useShortcuts` só registra seus atalhos; no conflito, vale o registrado por último
 * (a tela aberta sobre o shell).
 */
export function useShortcuts(bindings: Bindings): void {
  useEffect(() => {
    layers.push(bindings);
    install();
    return () => {
      const index = layers.lastIndexOf(bindings);
      if (index >= 0) layers.splice(index, 1);
    };
  }, [bindings]);
}

const layers: Bindings[] = [];
let installed = false;
let lastEscape = Number.NEGATIVE_INFINITY;

function install(): void {
  if (installed || typeof window === 'undefined') return;
  installed = true;
  // Captura: roda antes do textarea do xterm, que consumiria a tecla.
  window.addEventListener('keydown', onKeyDown, { capture: true });
}

function onKeyDown(event: KeyboardEvent): void {
  const target = event.target instanceof Element ? event.target : null;
  // Diálogos (formulários, confirmações) têm o próprio teclado.
  if (target?.closest('[role="dialog"], [role="alertdialog"]')) return;
  const terminal = target?.closest('.xterm') ?? null;

  if (terminal && leaveTerminal(event, terminal)) return;

  const combo = comboOf(event);
  if (!combo) return;
  for (let i = layers.length - 1; i >= 0; i--) {
    const binding = layers[i]?.[combo];
    if (!binding) continue;
    const shortcut = normalize(binding);
    if (!applies(shortcut, terminal !== null, isMacPlatform())) return;
    event.preventDefault();
    event.stopPropagation();
    shortcut.run(event);
    return;
  }
}

/**
 * `Esc Esc` sai do terminal para a navegação da UI (docs/08, acessibilidade). A primeira
 * `Esc` segue para o processo — vim e afins precisam dela —; a segunda, não.
 */
function leaveTerminal(event: KeyboardEvent, terminal: Element): boolean {
  if (event.key !== 'Escape' || event.metaKey || event.ctrlKey || event.shiftKey) return false;
  const now = event.timeStamp || performance.now();
  if (now - lastEscape > DOUBLE_ESCAPE_MS) {
    lastEscape = now;
    return false;
  }
  lastEscape = Number.NEGATIVE_INFINITY;
  event.preventDefault();
  event.stopPropagation();
  const pane = terminal.closest<HTMLElement>('[data-agent-pane]');
  if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  pane?.focus();
  return true;
}
