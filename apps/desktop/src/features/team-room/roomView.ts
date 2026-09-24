/** Vistas da Sala da Equipe (docs/09, T4.1, T4.2, T4.4 e o quadro T8). */
export const ROOM_VIEWS = ['grid', 'focus', 'timeline', 'board'] as const;
export type RoomView = (typeof ROOM_VIEWS)[number];

/** Lê `teams.layout.view`; ausente ou desconhecido (vista de uma fase futura) vira Grid. */
export function readRoomView(teamLayout: unknown): RoomView {
  const view =
    typeof teamLayout === 'object' && teamLayout !== null && 'view' in teamLayout
      ? (teamLayout as { view: unknown }).view
      : undefined;
  return ROOM_VIEWS.includes(view as RoomView) ? (view as RoomView) : 'grid';
}

/** Próxima vista no ciclo (o `⌘G` da F03-08 usa isto). */
export function nextRoomView(view: RoomView): RoomView {
  return ROOM_VIEWS[(ROOM_VIEWS.indexOf(view) + 1) % ROOM_VIEWS.length] ?? 'grid';
}

/**
 * Quem aparece no painel grande da vista Foco: o agente em foco, se ainda existe;
 * senão o primeiro da ordem.
 */
export function focusTarget(order: string[], focusedId: string | null): string | null {
  if (focusedId && order.includes(focusedId)) return focusedId;
  return order[0] ?? null;
}
