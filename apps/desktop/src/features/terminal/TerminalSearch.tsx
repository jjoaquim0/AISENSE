import type { SearchAddon } from '@xterm/addon-search';
import { ChevronDown, ChevronUp, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { IconButton } from '@/components/ui';

interface TerminalSearchProps {
  addon: SearchAddon;
  onClose: () => void;
}

/** Busca no histórico do terminal focado (⌘F). */
export function TerminalSearch({ addon, onClose }: TerminalSearchProps) {
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState('');

  useEffect(() => {
    input.current?.focus();
  }, []);

  const find = (direction: 'next' | 'previous'): void => {
    if (!query) return;
    const options = { caseSensitive: false, regex: false };
    if (direction === 'next') addon.findNext(query, options);
    else addon.findPrevious(query, options);
  };

  return (
    <div className="absolute top-2 right-2 z-10 flex items-center gap-1 rounded-md border border-subtle bg-raised p-1 shadow-md">
      <input
        ref={input}
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') find(event.shiftKey ? 'previous' : 'next');
          if (event.key === 'Escape') onClose();
        }}
        placeholder="Buscar no terminal"
        aria-label="Buscar no terminal"
        className="h-6 w-48 bg-transparent px-1.5 text-body text-primary outline-none placeholder:text-muted"
      />
      <IconButton label="Ocorrência anterior" size="sm" onClick={() => find('previous')}>
        <ChevronUp size={13} />
      </IconButton>
      <IconButton label="Próxima ocorrência" size="sm" onClick={() => find('next')}>
        <ChevronDown size={13} />
      </IconButton>
      <IconButton label="Fechar busca" size="sm" onClick={onClose}>
        <X size={13} />
      </IconButton>
    </div>
  );
}
