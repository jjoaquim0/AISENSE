import { useCallback, useEffect, useRef, useState } from 'react';
import { type Point, readFlowPositions } from '@/features/flow/flowModel';
import { errorMessage, teamsApi } from '@/features/teams/api';
import type { Team } from '@/types/generated/Team';
import { type GridLayout, readGridLayout, writeGridLayout } from '../gridLayout';
import { type RoomView, readRoomView } from '../roomView';

/**
 * Onde a linha do tempo estava (F08-06): a mensagem no topo da tela e quanto dela já tinha
 * passado. `null` = no fim, acompanhando as novas — o padrão.
 */
export interface TimelineScroll {
  anchor: string;
  delta: number;
}

export function readTimelineScroll(teamLayout: unknown): TimelineScroll | null {
  const t =
    typeof teamLayout === 'object' && teamLayout !== null && 'timeline' in teamLayout
      ? (teamLayout as { timeline: unknown }).timeline
      : null;
  if (typeof t !== 'object' || t === null) return null;
  const { anchor, delta } = t as { anchor?: unknown; delta?: unknown };
  return typeof anchor === 'string' && typeof delta === 'number' ? { anchor, delta } : null;
}

/** Agente em foco quando a equipe foi fechada (F08-06). */
export function readFocused(teamLayout: unknown): string | null {
  const f =
    typeof teamLayout === 'object' && teamLayout !== null && 'focused' in teamLayout
      ? (teamLayout as { focused: unknown }).focused
      : null;
  return typeof f === 'string' ? f : null;
}

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
  const [flow, setFlowState] = useState<Record<string, Point>>(() =>
    readFlowPositions(team.layout),
  );
  const [focused, setFocusedState] = useState<string | null>(() => readFocused(team.layout));
  const [timeline] = useState<TimelineScroll | null>(() => readTimelineScroll(team.layout));
  const saved = useRef<unknown>(team.layout);
  const pending = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const latest = useRef({ grid, view, flow, focused, timeline });
  latest.current = { ...latest.current, grid, view, flow, focused };
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
      flow: { positions: latest.current.flow },
      focused: latest.current.focused,
      timeline: latest.current.timeline,
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

  /** Posições arrastadas no Fluxo; `{}` volta ao layout automático. */
  const setFlowPositions = useCallback(
    (next: Record<string, Point>) => {
      latest.current = { ...latest.current, flow: next };
      setFlowState(next);
      schedule();
    },
    [schedule],
  );

  const setFocused = useCallback(
    (next: string | null) => {
      if (next === latest.current.focused) return;
      latest.current = { ...latest.current, focused: next };
      setFocusedState(next);
      schedule();
    },
    [schedule],
  );

  /** Rolagem da linha do tempo: só grava, não re-renderiza (muda a cada rolada). */
  const setTimelineScroll = useCallback(
    (next: TimelineScroll | null) => {
      const current = latest.current.timeline;
      if (current?.anchor === next?.anchor && current?.delta === next?.delta) return;
      latest.current = { ...latest.current, timeline: next };
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

  return {
    grid,
    setGrid,
    view,
    setView,
    flowPositions: flow,
    setFlowPositions,
    focused,
    setFocused,
    /** Como a linha do tempo estava ao abrir a equipe. */
    initialTimeline: timeline,
    setTimelineScroll,
  };
}

function hasSavedPreset(teamLayout: unknown): boolean {
  const grid =
    typeof teamLayout === 'object' && teamLayout !== null && 'grid' in teamLayout
      ? (teamLayout as { grid: unknown }).grid
      : undefined;
  return typeof grid === 'object' && grid !== null && 'preset' in grid;
}
