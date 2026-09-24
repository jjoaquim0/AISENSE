import { Command } from 'cmdk';
import { Send } from 'lucide-react';
import { useMemo, useState } from 'react';
import { formatShortcut } from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import { grouped, parseSend, pushRecent, readRecent, recentActions } from './paletteModel';
import { type PaletteAction, usePalette } from './paletteStore';

const item =
  'flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-body text-primary data-[selected=true]:bg-active';

/**
 * Paleta de comandos (T10, `⌘K`): navegação, criação, agentes, mensagens, quadro, vistas e
 * configuração. Busca difusa do `cmdk`; recentes no topo; `> enviar @alguem texto` manda.
 */
export function CommandPalette({ global }: { global: PaletteAction[] }) {
  const open = usePalette((s) => s.open);
  const setOpen = usePalette((s) => s.setOpen);
  const sources = usePalette((s) => s.sources);
  const send = usePalette((s) => s.send);
  const [query, setQuery] = useState('');
  const [recent, setRecent] = useState<string[]>(readRecent);
  const [problem, setProblem] = useState<string | null>(null);

  const actions = useMemo(() => [...Object.values(sources).flat(), ...global], [sources, global]);
  const recents = recentActions(actions, recent);
  const sending = parseSend(query);

  const close = () => {
    setOpen(false);
    setQuery('');
    setProblem(null);
  };

  const run = async (action: PaletteAction) => {
    setRecent((r) => pushRecent(action.id, r));
    close();
    try {
      await action.run();
    } catch (e: unknown) {
      // A paleta já fechou: o erro vai para o console da janela e a tela mostra o dela.
      console.error(errorMessage(e));
    }
  };

  const doSend = async () => {
    if (!sending || !send) return;
    try {
      await send(sending.to, sending.body);
      close();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  const row = (action: PaletteAction, prefix: string) => (
    <Command.Item
      key={`${prefix}:${action.id}`}
      value={`${prefix}:${action.id} ${action.label}`}
      keywords={action.keywords}
      onSelect={() => void run(action)}
      className={item}
    >
      <span className="min-w-0 flex-1 truncate">{action.label}</span>
      {action.shortcut && (
        <kbd className="font-sans text-caption text-muted">{formatShortcut(action.shortcut)}</kbd>
      )}
    </Command.Item>
  );

  return (
    <Command.Dialog
      open={open}
      onOpenChange={(next) => (next ? setOpen(true) : close())}
      label="Paleta de comandos"
      overlayClassName="fixed inset-0 z-40 bg-black/40"
      contentClassName="fixed top-[15vh] left-1/2 z-50 w-[min(36rem,calc(100vw-2rem))] -translate-x-1/2 overflow-hidden rounded-xl border border-subtle bg-raised shadow-lg"
      shouldFilter={!sending}
    >
      <Command.Input
        value={query}
        onValueChange={setQuery}
        placeholder="Buscar ação… ou > enviar @agente texto"
        className="h-11 w-full border-b border-subtle bg-transparent px-3 text-body text-primary outline-none placeholder:text-muted"
      />
      <Command.List className="max-h-[50vh] overflow-y-auto p-1.5">
        {sending ? (
          <Command.Item value="enviar" onSelect={() => void doSend()} className={item}>
            <Send size={13} />
            <span className="min-w-0 flex-1 truncate">
              {send
                ? `Enviar para ${sending.to}: ${sending.body}`
                : 'Abra uma equipe para enviar mensagens'}
            </span>
          </Command.Item>
        ) : (
          <>
            <Command.Empty className="px-2 py-3 text-caption text-muted">
              Nada encontrado.
            </Command.Empty>
            {recents.length > 0 && !query && (
              <Command.Group heading="Recentes" className="text-caption text-muted">
                {recents.map((a) => row(a, 'recente'))}
              </Command.Group>
            )}
            {grouped(actions).map(([group, list]) => (
              <Command.Group key={group} heading={group} className="text-caption text-muted">
                {list.map((a) => row(a, group))}
              </Command.Group>
            ))}
          </>
        )}
        {problem && (
          <p role="alert" className="px-2 py-1 text-caption text-failed">
            {problem}
          </p>
        )}
      </Command.List>
    </Command.Dialog>
  );
}
