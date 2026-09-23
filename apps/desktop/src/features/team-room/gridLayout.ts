import { z } from 'zod';

/** Layouts da vista Grid (docs/09, T4.1): quantidade fixa de painéis ou livre. */
export const GRID_PRESETS = ['1', '2', '3', '4', '6', '9', 'free'] as const;
export type GridPreset = (typeof GRID_PRESETS)[number];

/** Colunas × linhas de cada preset. */
export const PRESET_SHAPE: Record<Exclude<GridPreset, 'free'>, [number, number]> = {
  '1': [1, 1],
  '2': [2, 1],
  '3': [3, 1],
  '4': [2, 2],
  '6': [3, 2],
  '9': [3, 3],
};

/** O modo livre usa uma grade invisível de 24×24 células: arrastar encaixa nela. */
export const FREE_UNITS = 24;
/** Menor painel no modo livre, em células: abaixo disso um terminal é ilegível. */
export const FREE_MIN = 4;

export interface FreeRect {
  x: number;
  y: number;
  w: number;
  h: number;
  /** Ordem de empilhamento: o último painel mexido fica por cima dos outros. */
  z?: number;
}

export interface GridLayout {
  preset: GridPreset;
  /** Ordem dos painéis (ids de agente). Nos presets, só os N primeiros aparecem. */
  order: string[];
  /** Posição de cada painel no modo livre, em células de `FREE_UNITS`. */
  free: Record<string, FreeRect>;
}

const rectSchema = z.object({
  x: z.number().int(),
  y: z.number().int(),
  w: z.number().int(),
  h: z.number().int(),
  z: z.number().int().optional(),
});

const savedSchema = z.object({
  preset: z.enum(GRID_PRESETS).optional(),
  order: z.array(z.string()).optional(),
  free: z.record(z.string(), rectSchema).optional(),
});

/** Preset inicial: o menor que mostra todos os agentes (até 9). */
export function presetFor(count: number): GridPreset {
  for (const preset of ['1', '2', '3', '4', '6', '9'] as const) {
    if (count <= Number(preset)) return preset;
  }
  return '9';
}

export function slotsOf(preset: GridPreset): number {
  return preset === 'free' ? Number.POSITIVE_INFINITY : Number(preset);
}

/**
 * Lê `teams.layout.grid` e o reconcilia com os agentes que existem agora: quem sumiu
 * sai da ordem, quem é novo entra no fim. Layout salvo inválido (versão antiga, edição
 * à mão) não quebra a tela — vira o padrão.
 */
export function readGridLayout(teamLayout: unknown, agentIds: string[]): GridLayout {
  const raw =
    typeof teamLayout === 'object' && teamLayout !== null && 'grid' in teamLayout
      ? (teamLayout as { grid: unknown }).grid
      : undefined;
  const parsed = savedSchema.safeParse(raw);
  const saved = parsed.success ? parsed.data : {};
  const known = new Set(agentIds);
  const kept = (saved.order ?? []).filter((id, i, all) => known.has(id) && all.indexOf(id) === i);
  const order = [...kept, ...agentIds.filter((id) => !kept.includes(id))];
  const free: Record<string, FreeRect> = {};
  for (const [id, rect] of Object.entries(saved.free ?? {})) {
    if (known.has(id)) free[id] = clampRect(rect);
  }
  return { preset: saved.preset ?? presetFor(agentIds.length), order, free };
}

/** Grava o grid dentro de `teams.layout` sem apagar o que outras vistas guardam lá. */
export function writeGridLayout(teamLayout: unknown, grid: GridLayout): Record<string, unknown> {
  const base =
    typeof teamLayout === 'object' && teamLayout !== null && !Array.isArray(teamLayout)
      ? (teamLayout as Record<string, unknown>)
      : {};
  return { ...base, grid };
}

export function visibleIds(layout: GridLayout): string[] {
  return layout.order.slice(0, slotsOf(layout.preset));
}

export function hiddenIds(layout: GridLayout): string[] {
  return layout.order.slice(slotsOf(layout.preset));
}

/** Arrastar um painel sobre outro: o arrastado assume a posição do alvo. */
export function reorder(order: string[], activeId: string, overId: string): string[] {
  const from = order.indexOf(activeId);
  const to = order.indexOf(overId);
  if (from < 0 || to < 0 || from === to) return order;
  const next = [...order];
  next.splice(from, 1);
  next.splice(to, 0, activeId);
  return next;
}

/** "Mostrar" um painel oculto: ele entra no último lugar visível. */
export function promote(layout: GridLayout, id: string): GridLayout {
  const slots = slotsOf(layout.preset);
  const index = layout.order.indexOf(id);
  if (index < 0 || index < slots) return layout;
  const order = layout.order.filter((x) => x !== id);
  order.splice(slots - 1, 0, id);
  return { ...layout, order };
}

/** Posição no modo livre; quem ainda não tem uma ganha um lugar em mosaico 3×3. */
export function freeRectOf(layout: GridLayout, id: string): FreeRect {
  const saved = layout.free[id];
  if (saved) return saved;
  const index = Math.max(0, layout.order.indexOf(id));
  const size = FREE_UNITS / 3;
  return { x: (index % 3) * size, y: (Math.floor(index / 3) % 3) * size, w: size, h: size };
}

/** Mantém o painel dentro da área e com tamanho mínimo. */
export function clampRect(rect: FreeRect): FreeRect {
  const w = Math.min(FREE_UNITS, Math.max(FREE_MIN, Math.round(rect.w)));
  const h = Math.min(FREE_UNITS, Math.max(FREE_MIN, Math.round(rect.h)));
  const x = Math.min(FREE_UNITS - w, Math.max(0, Math.round(rect.x)));
  const y = Math.min(FREE_UNITS - h, Math.max(0, Math.round(rect.y)));
  return rect.z === undefined ? { x, y, w, h } : { x, y, w, h, z: rect.z };
}

/** Grava a nova posição e traz o painel para a frente: soltar um painel sobre outro
 *  não pode escondê-lo atrás. */
export function setFreeRect(layout: GridLayout, id: string, rect: FreeRect): GridLayout {
  const top = Math.max(0, ...Object.values(layout.free).map((r) => r.z ?? 0));
  return { ...layout, free: { ...layout.free, [id]: clampRect({ ...rect, z: top + 1 }) } };
}

const FIXED_PRESETS = ['1', '2', '3', '4', '6', '9'] as const;

/**
 * `⌘W` — fechar o painel (docs/08): sai da grade, **o agente continua rodando**. O
 * painel vai para "Fora da grade" e o preset encolhe para o menor que ainda mostra os
 * outros visíveis. Último painel na tela, ou modo livre (onde todos aparecem), não fecha.
 */
export function closePane(layout: GridLayout, id: string): GridLayout {
  if (layout.preset === 'free') return layout;
  const visible = visibleIds(layout);
  if (!visible.includes(id) || visible.length <= 1) return layout;
  const order = [...layout.order.filter((x) => x !== id), id];
  const remaining = visible.length - 1;
  const preset = FIXED_PRESETS.find((p) => Number(p) >= remaining) ?? '9';
  return { ...layout, preset, order };
}

/** `⌘\` — dividir: o próximo preset maior, que abre lugar para mais um painel. */
export function splitLayout(layout: GridLayout): GridLayout {
  if (layout.preset === 'free') return layout;
  const next = FIXED_PRESETS[FIXED_PRESETS.indexOf(layout.preset) + 1];
  return next ? { ...layout, preset: next } : layout;
}
