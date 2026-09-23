import { type RefObject, useEffect, useState } from 'react';

/**
 * `true` quando o elemento tem algum pixel na tela **e** a janela está em primeiro
 * plano. Um painel rolado para fora, coberto por outra vista ou com o app minimizado
 * não precisa receber saída ao vivo (F03-05).
 */
export function useOnScreen(ref: RefObject<HTMLElement | null>): boolean {
  const [intersecting, setIntersecting] = useState(true);
  const [pageVisible, setPageVisible] = useState(() => !document.hidden);

  useEffect(() => {
    const element = ref.current;
    if (!element || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(([entry]) => {
      if (entry) setIntersecting(entry.isIntersecting);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);

  useEffect(() => {
    const update = () => setPageVisible(!document.hidden);
    document.addEventListener('visibilitychange', update);
    return () => document.removeEventListener('visibilitychange', update);
  }, []);

  return intersecting && pageVisible;
}
