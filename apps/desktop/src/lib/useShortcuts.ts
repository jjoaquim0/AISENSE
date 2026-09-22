import { useEffect } from 'react';

type Handler = (event: KeyboardEvent) => void;

/**
 * Atalhos globais. A chave é `⌘X`, `⌘⇧X` ou apenas `X`; `⌘` casa com Cmd no macOS
 * e Ctrl no resto (docs/08-design-system.md).
 *
 * Um único listener no documento — `listen()` espalhado por componente vaza handler
 * e vira bug de memória com muitos painéis abertos (docs/10-padroes-de-codigo.md).
 */
export function useShortcuts(bindings: Record<string, Handler>): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const mod = event.metaKey || event.ctrlKey;
      const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
      const combo = `${mod ? '⌘' : ''}${event.shiftKey ? '⇧' : ''}${key}`;
      const handler = bindings[combo];
      if (!handler) return;
      event.preventDefault();
      handler(event);
    };

    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [bindings]);
}
