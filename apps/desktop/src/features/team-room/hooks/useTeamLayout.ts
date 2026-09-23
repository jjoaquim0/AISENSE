import { useCallback, useEffect, useRef, useState } from 'react';
import { errorMessage, teamsApi } from '@/features/teams/api';
import type { Team } from '@/types/generated/Team';
import { type GridLayout, readGridLayout, writeGridLayout } from '../gridLayout';
import { type RoomView, readRoomView } from '../roomView';

/** Espera entre o último arraste e a gravação: arrastar não vira uma rajada de escritas. */
const SAVE_DELAY_MS = 400;

/**
 * Estado da Sala da Equipe guardado em `teams.layout`: a vista escolhida e o layout da
 * Grid. Um único dono grava o objeto inteiro — dois hooks gravando chaves diferentes
 * do mesmo JSON apagariam um ao outro. Agentes novos ou removidos entram/saem da
 * ordem sem perder o resto.
 */
export function useTeamLayout(team: Team, agentIds: string[], onError: (text: string) => void) {
  const idsKey = agentIds.join('|');
  const [grid, setGridState] = useState<GridLayout>(() => readGridLayout(team.layout, agentIds));
  const [view, setViewState] = useState<RoomView>(() => readRoomView(team.layout));
  const saved = useRef<unknown>(team.layout);
  const pending = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const latest = useRef({ grid, view });
  latest.current = { grid, view };
  // Sem preset salvo nem escolhido, o preset acompanha a quantidade de agentes. Sem
  // isto, a equipe abria com a lista ainda vazia, ganhava "1" e ficava presa nele.
  const presetChosen = useRef(hasSavedPreset(team.layout));

  // Reconciliar quando a lista de agentes muda (criou, duplicou, excluiu).
  useEffect(() => {
    const ids = idsKey ? idsKey.split('|') : [];
    setGridState((current) =>
      readGridLayout(
        { grid: presetChosen.current ? current : { ...current, preset: undefined } },
        ids,
      ),
    );
  }, [idsKey]);

  const persist = useCallback(() => {
    pending.current = undefined;
    const body = {
      ...writeGridLayout(saved.current, latest.current.grid),
      view: latest.current.view,
    };
    saved.current = body;
    void teamsApi.setLayout(team.id, body).catch((e: unknown) => onError(errorMessage(e)));
  }, [team.id, onError]);

  const schedule = useCallback(() => {
    clearTimeout(pending.current);
    pending.current = setTimeout(persist, SAVE_DELAY_MS);
  }, [persist]);

  const setGrid = useCallback(
    (next: GridLayout) => {
      if (next.preset !== latest.current.grid.preset) presetChosen.current = true;
      latest.current = { ...latest.current, grid: next };
      setGridState(next);
      schedule();
    },
    [schedule],
  );

  const setView = useCallback(
    (next: RoomView) => {
      latest.current = { ...latest.current, view: next };
      setViewState(next);
      schedule();
    },
    [schedule],
  );

  // Sair da tela com uma gravação pendente: grava agora em vez de perder.
  useEffect(
    () => () => {
      if (pending.current) {
        clearTimeout(pending.current);
        persist();
      }
    },
    [persist],
  );

  return { grid, setGrid, view, setView };
}

function hasSavedPreset(teamLayout: unknown): boolean {
  const grid =
    typeof teamLayout === 'object' && teamLayout !== null && 'grid' in teamLayout
      ? (teamLayout as { grid: unknown }).grid
      : undefined;
  return typeof grid === 'object' && grid !== null && 'preset' in grid;
}
