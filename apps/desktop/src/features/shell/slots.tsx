import { type ReactNode, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { create } from 'zustand';

/** Regiões da janela que a tela aberta preenche (docs/09, "Estrutura global da janela"). */
export type SlotName = 'sidebar' | 'inspector';

interface SlotState {
  hosts: Partial<Record<SlotName, HTMLElement | null>>;
  /** Quantos conteúdos ocupam cada região; zero mostra o texto padrão do shell. */
  filled: Partial<Record<SlotName, number>>;
  setHost: (name: SlotName, element: HTMLElement | null) => void;
  occupy: (name: SlotName, delta: 1 | -1) => void;
}

export const useShellSlots = create<SlotState>((set) => ({
  hosts: {},
  filled: {},
  setHost: (name, element) =>
    set((s) => (s.hosts[name] === element ? s : { hosts: { ...s.hosts, [name]: element } })),
  occupy: (name, delta) =>
    set((s) => ({ filled: { ...s.filled, [name]: (s.filled[name] ?? 0) + delta } })),
}));

/**
 * Coloca `children` numa região do shell (sidebar, inspetor) sem tirar o estado de quem
 * renderiza: a Sala da Equipe continua dona dos agentes e da seleção, e a lista aparece
 * onde o doc 09 manda. Com a região oculta (⌘B, ⌘I) nada é renderizado.
 */
export function ShellSlot({ name, children }: { name: SlotName; children: ReactNode }) {
  const host = useShellSlots((s) => s.hosts[name]);
  const occupy = useShellSlots((s) => s.occupy);
  useEffect(() => {
    occupy(name, 1);
    return () => occupy(name, -1);
  }, [name, occupy]);
  return host ? createPortal(children, host) : null;
}
