import { cn } from '@/lib/cn';
import { GRID_PRESETS, type GridPreset } from '../gridLayout';

const LABEL: Record<GridPreset, string> = {
  '1': '1',
  '2': '2',
  '3': '3',
  '4': '4',
  '6': '6',
  '9': '9',
  free: 'Livre',
};

/** Seletor de layout da vista Grid: `1 · 2 · 3 · 4 · 6 · 9 · Livre`. */
export function PresetPicker({
  value,
  onChange,
}: {
  value: GridPreset;
  onChange: (preset: GridPreset) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label="Layout da grade"
      className="flex items-center rounded-md border border-subtle p-0.5"
    >
      {GRID_PRESETS.map((preset) => (
        // biome-ignore lint/a11y/useSemanticElements: botões segmentados com papel de rádio, padrão WAI-ARIA
        <button
          key={preset}
          type="button"
          role="radio"
          aria-checked={value === preset}
          aria-label={preset === 'free' ? 'Layout livre' : `${preset} painéis`}
          onClick={() => onChange(preset)}
          className={cn(
            'min-w-6 rounded px-1.5 py-0.5 text-caption tabular-nums',
            value === preset ? 'bg-active text-primary' : 'text-muted hover:text-primary',
          )}
        >
          {LABEL[preset]}
        </button>
      ))}
    </div>
  );
}
