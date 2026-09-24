import { type RefObject, useCallback, useEffect, useMemo, useRef, useState } from 'react';

/** Altura estimada de uma mensagem antes de medir. */
export const ESTIMATED_ROW = 72;
/** Quanto além da tela ainda é renderizado (px), para a rolagem não mostrar buraco. */
const OVERSCAN = 600;

/** Posição de cada item a partir das alturas conhecidas (ou estimadas). */
export function offsetsOf(keys: string[], heights: Map<string, number>): number[] {
  const offsets = new Array<number>(keys.length + 1);
  offsets[0] = 0;
  for (let i = 0; i < keys.length; i++) {
    offsets[i + 1] = (offsets[i] ?? 0) + (heights.get(keys[i] ?? '') ?? ESTIMATED_ROW);
  }
  return offsets;
}

/** Primeiro índice cujo fim passa de `y` (busca binária). */
export function indexAt(offsets: number[], y: number): number {
  let lo = 0;
  let hi = offsets.length - 2;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if ((offsets[mid + 1] ?? 0) <= y) lo = mid + 1;
    else hi = mid;
  }
  return Math.max(0, lo);
}

/**
 * Lista virtual com alturas variáveis: só o que está perto da tela é montado; o resto vira
 * dois espaçadores. As alturas reais são medidas por `ResizeObserver` e guardadas por
 * chave — então mensagens novas no começo (histórico) ou no fim não bagunçam as outras.
 */
export function useVirtualList(keys: string[], container: RefObject<HTMLElement | null>) {
  const heights = useRef(new Map<string, number>());
  const [version, setVersion] = useState(0);
  const [view, setView] = useState({ top: 0, height: 800 });

  // biome-ignore lint/correctness/useExhaustiveDependencies: `version` marca alturas novas
  const offsets = useMemo(() => offsetsOf(keys, heights.current), [keys, version]);
  const total = offsets[keys.length] ?? 0;

  const sync = useCallback(() => {
    const el = container.current;
    if (el) setView({ top: el.scrollTop, height: el.clientHeight });
  }, [container]);

  useEffect(() => {
    const el = container.current;
    if (!el) return;
    let frame = 0;
    const onScroll = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(sync);
    };
    el.addEventListener('scroll', onScroll, { passive: true });
    const resize = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(sync);
    resize?.observe(el);
    sync();
    return () => {
      cancelAnimationFrame(frame);
      el.removeEventListener('scroll', onScroll);
      resize?.disconnect();
    };
  }, [container, sync]);

  const first = keys.length === 0 ? 0 : indexAt(offsets, Math.max(0, view.top - OVERSCAN));
  const last = keys.length === 0 ? -1 : indexAt(offsets, view.top + view.height + OVERSCAN);

  // Mede quem está montado; muda a versão só quando alguma altura mudou de fato.
  const observer = useMemo(() => {
    if (typeof ResizeObserver === 'undefined') return null;
    let pending = false;
    return new ResizeObserver((entries) => {
      for (const entry of entries) {
        const key = (entry.target as HTMLElement).dataset.key;
        const h = Math.ceil(entry.borderBoxSize?.[0]?.blockSize ?? entry.contentRect.height);
        if (key && h > 0 && heights.current.get(key) !== h) {
          heights.current.set(key, h);
          pending = true;
        }
      }
      if (pending) {
        pending = false;
        setVersion((v) => v + 1);
      }
    });
  }, []);
  useEffect(() => () => observer?.disconnect(), [observer]);

  const measure = useCallback(
    (el: HTMLElement | null) => {
      if (el && observer) observer.observe(el);
    },
    [observer],
  );

  /** Onde começa o item `key`, e qual item está na altura `y` (âncora da F08-06). */
  const offsetOf = useCallback(
    (key: string): number | null => {
      const index = keys.indexOf(key);
      return index < 0 ? null : (offsets[index] ?? null);
    },
    [keys, offsets],
  );
  const keyAt = useCallback(
    (y: number): { key: string; offset: number } | null => {
      if (keys.length === 0) return null;
      const index = indexAt(offsets, y);
      const key = keys[index];
      return key === undefined ? null : { key, offset: offsets[index] ?? 0 };
    },
    [keys, offsets],
  );

  return {
    first,
    last,
    offsetOf,
    keyAt,
    before: offsets[first] ?? 0,
    after: total - (offsets[last + 1] ?? total),
    total,
    measure,
  };
}
