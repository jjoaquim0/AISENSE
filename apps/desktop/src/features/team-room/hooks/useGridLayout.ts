import { useCallback, useEffect, useRef, useState } from 'react';
import { errorMessage, teamsApi } from '@/features/teams/api';
import type { Team } from '@/types/generated/Team';
import { type GridLayout, readGridLayout, writeGridLayout } from '../gridLayout';

/** Espera entre o último arraste e a gravação: arrastar não vira uma rajada de escritas. */
const SAVE_DELAY_MS = 400;

/**
 * Layout da vista Grid de uma equipe, lido de `teams.layout` e gravado de volta com
 * atraso. Agentes novos ou removidos entram/saem da ordem sem perder o resto.
 */
export function useGridLayout(team: Team, agentIds: string[], onError: (text: string) => void) {
  const idsKey = agentIds.join('|');
  const [layout, setLayout] = useState<GridLayout>(() => readGridLayout(team.layout, agentIds));
  const saved = useRef<unknown>(team.layout);
  const pending = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Reconciliar quando a lista de agentes muda (criou, duplicou, excluiu).
  useEffect(() => {
    setLayout((current) => readGridLayout({ grid: current }, idsKey ? idsKey.split('|') : []));
  }, [idsKey]);

  const persist = useCallback(
    (next: GridLayout) => {
      const body = writeGridLayout(saved.current, next);
      saved.current = body;
      void teamsApi.setLayout(team.id, body).catch((e: unknown) => onError(errorMessage(e)));
    },
    [team.id, onError],
  );

  const change = useCallback(
    (next: GridLayout) => {
      setLayout(next);
      clearTimeout(pending.current);
      pending.current = setTimeout(() => persist(next), SAVE_DELAY_MS);
    },
    [persist],
  );

  // Sair da tela com uma gravação pendente: grava agora em vez de perder.
  const latest = useRef(layout);
  latest.current = layout;
  useEffect(
    () => () => {
      if (pending.current) {
        clearTimeout(pending.current);
        persist(latest.current);
      }
    },
    [persist],
  );

  return [layout, change] as const;
}
