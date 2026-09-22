import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

/** Tecla de atalho. Use `⌘` — a troca por `Ctrl` fora do macOS é feita em `formatShortcut`. */
export function Kbd({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <kbd
      className={cn(
        'inline-flex h-5 min-w-5 items-center justify-center rounded-sm border border-subtle',
        'bg-surface px-1 font-sans text-caption text-muted',
        className,
      )}
    >
      {children}
    </kbd>
  );
}

const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);

/** `⌘K` no macOS, `Ctrl+K` no resto. */
export function formatShortcut(shortcut: string): string {
  return isMac ? shortcut : shortcut.replace('⌘', 'Ctrl+').replace('⇧', 'Shift+');
}
