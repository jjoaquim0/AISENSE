import type { Bindings } from '@/lib/shortcuts';
import { closePane, type GridLayout, promote, splitLayout, visibleIds } from './gridLayout';
import { nextRoomView, type RoomView } from './roomView';

export interface RoomContext {
  grid: GridLayout;
  view: RoomView;
  selectedId: string | null;
  setGrid: (grid: GridLayout) => void;
  setView: (view: RoomView) => void;
  select: (id: string) => void;
  /** Leva o teclado para o terminal do agente (ou o painel, se parado). */
  focusPane: (id: string) => void;
  newAgent: () => void;
}

/**
 * Atalhos da Sala da Equipe (docs/08). A numeração de `⌘1..9` é a ordem dos painéis
 * (`grid.order`), a mesma das miniaturas da vista Foco. Nenhum atalho para ou recria
 * processo: fechar um painel só o tira da tela.
 */
export function roomShortcuts(room: RoomContext): Bindings {
  const bindings: Bindings = {};

  for (let n = 1; n <= 9; n++) {
    bindings[`⌘${n}`] = {
      // Ctrl+dígito não é tecla de shell: vale até com o terminal focado, em todo SO.
      inTerminal: 'always',
      run: () => {
        const id = room.grid.order[n - 1];
        if (!id) return;
        room.select(id);
        if (room.view === 'grid' && !visibleIds(room.grid).includes(id)) {
          room.setGrid(promote(room.grid, id));
        }
        room.focusPane(id);
      },
    };
  }

  bindings['⌘G'] = () => room.setView(nextRoomView(room.view));
  bindings['⌘T'] = () => room.newAgent();

  bindings['⌘W'] = () => {
    const id = room.selectedId;
    if (room.view !== 'grid' || !id) return;
    const next = closePane(room.grid, id);
    if (next === room.grid) return;
    room.setGrid(next);
    // O foco estava no painel que saiu; vai para o primeiro que ficou.
    const first = visibleIds(next)[0];
    if (first) {
      room.select(first);
      room.focusPane(first);
    }
  };

  bindings['⌘\\'] = () => {
    if (room.view === 'focus') room.setView('grid');
    else room.setGrid(splitLayout(room.grid));
  };

  return bindings;
}
