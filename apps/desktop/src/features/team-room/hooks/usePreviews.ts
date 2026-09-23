import { useEffect, useState } from 'react';
import { agentsApi } from '@/features/agents/api';

/** 2 atualizações por segundo (docs/09, T4.2): prévia, não um terminal ao vivo. */
const PREVIEW_INTERVAL_MS = 500;
export const PREVIEW_LINES = 4;

/**
 * Últimas linhas da tela de cada agente, buscadas no core a 2 fps numa única
 * chamada para todos. Janela em segundo plano não busca nada.
 */
export function usePreviews(agentIds: string[]): Record<string, string[]> {
  const [previews, setPreviews] = useState<Record<string, string[]>>({});
  const key = agentIds.join('|');

  useEffect(() => {
    const ids = key ? key.split('|') : [];
    if (ids.length === 0) return;
    let alive = true;
    const refresh = () => {
      if (document.hidden) return;
      agentsApi
        .previews(ids, PREVIEW_LINES)
        .then((list) => {
          if (!alive) return;
          setPreviews((current) => {
            const next: Record<string, string[]> = {};
            for (const p of list) next[p.agentId] = p.lines;
            // Mesmo conteúdo: devolve o objeto antigo e nada re-renderiza.
            return sameLines(current, next) ? current : next;
          });
        })
        .catch(() => {
          // Sem core (navegador puro) ou agente removido: a miniatura fica vazia.
        });
    };
    refresh();
    const timer = setInterval(refresh, PREVIEW_INTERVAL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [key]);

  return previews;
}

function sameLines(a: Record<string, string[]>, b: Record<string, string[]>): boolean {
  const keys = Object.keys(b);
  if (keys.length !== Object.keys(a).length) return false;
  return keys.every((k) => {
    const x = a[k];
    const y = b[k];
    return (
      x !== undefined && y !== undefined && x.length === y.length && x.every((l, i) => l === y[i])
    );
  });
}
