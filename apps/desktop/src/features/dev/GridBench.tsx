import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import { Terminal as Xterm } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import { useEffect, useRef, useState } from 'react';
import { GridView } from '@/features/team-room/components/GridView';
import { PresetPicker } from '@/features/team-room/components/PresetPicker';
import { type GridLayout, readGridLayout } from '@/features/team-room/gridLayout';

const IDS = Array.from({ length: 9 }, (_, i) => `agt_${i + 1}`);
const COLORS = [
  'violet',
  'cyan',
  'emerald',
  'amber',
  'rose',
  'indigo',
  'teal',
  'fuchsia',
  'violet',
];

declare global {
  interface Window {
    /** Quadros por segundo medidos a cada 500 ms, para a bancada automatizada ler. */
    __fps?: number[];
  }
}

/**
 * Bancada de desempenho da vista Grid (F03-03), só em desenvolvimento: `#/dev/grid`.
 * Nove xterm reais recebendo saída contínua, como nove agentes verbosos, dentro da
 * mesma `GridView` do app. O medidor no canto mostra os quadros por segundo.
 * Variantes para isolar custos: `?quiet` (terminais parados) e `?nogl` (sem WebGL).
 */
export function GridBench() {
  const [layout, setLayout] = useState<GridLayout>(() => readGridLayout({}, IDS));
  const fps = useFps();
  return (
    <div className="flex h-screen flex-col gap-2 bg-base p-2">
      <header className="flex items-center gap-3">
        <h1 className="text-heading text-primary">Bancada da grade</h1>
        <PresetPicker
          value={layout.preset}
          onChange={(preset) => setLayout({ ...layout, preset })}
        />
        <span className="ml-auto font-mono text-label text-primary tabular-nums" data-testid="fps">
          {fps} fps
        </span>
      </header>
      <GridView
        layout={layout}
        onChange={setLayout}
        labelOf={(id) => `@${id}`}
        renderPane={(id, drag) => (
          <div
            className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg border border-subtle border-l-[3px] bg-surface"
            style={{ borderLeftColor: `var(--agent-${COLORS[IDS.indexOf(id)]})` }}
          >
            <div
              ref={drag.setRef}
              {...drag.props}
              data-testid={`handle-${id}`}
              className="cursor-grab border-b border-subtle px-3 py-1.5 text-label text-primary"
            >
              @{id}
            </div>
            <BenchTerminal />
          </div>
        )}
      />
    </div>
  );
}

/** `#/dev/grid?quiet` sem saída contínua; `?nogl` sem WebGL — para isolar o custo do arraste. */
const OPTIONS = new URLSearchParams(window.location.hash.split('?')[1] ?? '');

function BenchTerminal() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const container = host.current;
    if (!container) return;
    const xterm = new Xterm({ fontSize: 12, scrollback: 2_000 });
    const fit = new FitAddon();
    xterm.loadAddon(fit);
    xterm.open(container);
    try {
      if (!OPTIONS.has('nogl')) xterm.loadAddon(new WebglAddon());
    } catch {
      // Sem WebGL, o renderer padrão — a medição fica pessimista, não otimista.
    }
    fit.fit();
    let n = 0;
    // ~60 linhas coloridas por segundo por terminal: um agente bem verboso.
    const timer = setInterval(() => {
      if (OPTIONS.has('quiet') && n > 40) return;
      n += 1;
      xterm.write(`\x1b[3${n % 7}m${n}\x1b[0m compilando módulo ${n} — ${'█'.repeat(n % 40)}\r\n`);
    }, 16);
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(container);
    return () => {
      clearInterval(timer);
      observer.disconnect();
      xterm.dispose();
    };
  }, []);
  return <div ref={host} className="min-h-0 flex-1 bg-terminal" />;
}

function useFps(): number {
  const [fps, setFps] = useState(0);
  useEffect(() => {
    let frames = 0;
    let last = performance.now();
    let raf = 0;
    window.__fps = [];
    const tick = (now: number) => {
      frames += 1;
      if (now - last >= 500) {
        const value = Math.round((frames * 1000) / (now - last));
        window.__fps?.push(value);
        setFps(value);
        frames = 0;
        last = now;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);
  return fps;
}
