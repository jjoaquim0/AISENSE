import { Moon, PanelRight, Plus, Sun } from 'lucide-react';
import { type ReactNode, useCallback, useMemo } from 'react';
import { IconButton, Tooltip } from '@/components/ui';
import { formatShortcut } from '@/components/ui/Kbd';
import { useTeams } from '@/features/teams/store';
import { isDark, useTheme } from '@/lib/theme';
import { useShortcuts } from '@/lib/useShortcuts';
import { ResizeHandle } from './ResizeHandle';
import { type SlotName, useShellSlots } from './slots';
import { INSPECTOR_BOUNDS, SIDEBAR_BOUNDS, usePanels } from './usePanels';

/**
 * Estrutura global da janela — docs/09-telas-e-fluxos.md:
 * trilho de equipes (48px) · sidebar · área principal · inspetor.
 */
export function AppShell({ children }: { children: ReactNode }) {
  const {
    sidebarWidth,
    inspectorWidth,
    sidebarVisible,
    inspectorVisible,
    setSidebarWidth,
    setInspectorWidth,
    toggleSidebar,
    toggleInspector,
  } = usePanels();
  const { toggle: toggleTheme } = useTheme();

  const shortcuts = useMemo(
    () => ({ '⌘B': toggleSidebar, '⌘I': toggleInspector, '⌘⇧D': toggleTheme }),
    [toggleSidebar, toggleInspector, toggleTheme],
  );
  useShortcuts(shortcuts);

  return (
    <div className="flex h-full flex-col bg-base text-primary">
      <TitleBar onToggleInspector={toggleInspector} inspectorVisible={inspectorVisible} />
      <div className="flex min-h-0 flex-1">
        <TeamRail />
        {sidebarVisible && (
          <>
            <Sidebar width={sidebarWidth} />
            <ResizeHandle
              value={sidebarWidth}
              onChange={setSidebarWidth}
              edge="right"
              min={SIDEBAR_BOUNDS.min}
              max={SIDEBAR_BOUNDS.max}
              label="Largura da lista de agentes"
            />
          </>
        )}
        <main className="min-w-0 flex-1 overflow-auto">{children}</main>
        {inspectorVisible && (
          <>
            <ResizeHandle
              value={inspectorWidth}
              onChange={setInspectorWidth}
              edge="left"
              min={INSPECTOR_BOUNDS.min}
              max={INSPECTOR_BOUNDS.max}
              label="Largura do inspetor"
            />
            <Inspector width={inspectorWidth} />
          </>
        )}
      </div>
    </div>
  );
}

function TitleBar({
  onToggleInspector,
  inspectorVisible,
}: {
  onToggleInspector: () => void;
  inspectorVisible: boolean;
}) {
  const { theme, toggle } = useTheme();
  const dark = isDark(theme);

  return (
    <header
      data-tauri-drag-region
      className="flex h-10 shrink-0 items-center justify-between border-b border-subtle bg-surface pr-2 pl-20"
    >
      <span className="text-label text-secondary">AISENSE</span>
      <div className="flex items-center gap-0.5">
        <Tooltip
          content={`${inspectorVisible ? 'Ocultar' : 'Mostrar'} inspetor · ${formatShortcut('⌘I')}`}
        >
          <IconButton
            label={inspectorVisible ? 'Ocultar inspetor' : 'Mostrar inspetor'}
            onClick={onToggleInspector}
            className={inspectorVisible ? 'text-primary' : undefined}
          >
            <PanelRight size={15} />
          </IconButton>
        </Tooltip>
        <Tooltip content={`Tema ${dark ? 'claro' : 'escuro'} · ${formatShortcut('⌘⇧D')}`}>
          <IconButton label={dark ? 'Usar tema claro' : 'Usar tema escuro'} onClick={toggle}>
            {dark ? <Sun size={15} /> : <Moon size={15} />}
          </IconButton>
        </Tooltip>
      </div>
    </header>
  );
}

function TeamRail() {
  const openWizard = useTeams((s) => s.setWizardOpen);
  return (
    <nav
      aria-label="Equipes"
      className="flex w-12 shrink-0 flex-col items-center gap-2 border-r border-subtle bg-surface py-3"
    >
      <Tooltip content="Nova equipe" side="right">
        <button
          type="button"
          aria-label="Nova equipe"
          onClick={() => openWizard(true)}
          className="flex size-8 items-center justify-center rounded-lg border border-dashed border-strong text-muted transition-colors duration-100 hover:border-accent hover:text-accent"
        >
          <Plus size={16} />
        </button>
      </Tooltip>
    </nav>
  );
}

/** Onde a tela aberta coloca seu conteúdo (`ShellSlot`); vazio, mostra `fallback`. */
function SlotHost({ name, fallback }: { name: SlotName; fallback: ReactNode }) {
  const setHost = useShellSlots((s) => s.setHost);
  const filled = useShellSlots((s) => (s.filled[name] ?? 0) > 0);
  const ref = useCallback(
    (element: HTMLDivElement | null) => setHost(name, element),
    [name, setHost],
  );
  return (
    <>
      {!filled && fallback}
      <div ref={ref} className="flex min-h-0 flex-1 flex-col" />
    </>
  );
}

function Sidebar({ width }: { width: number }) {
  return (
    <aside
      aria-label="Agentes da equipe"
      style={{ width }}
      className="flex shrink-0 flex-col border-r border-subtle bg-surface"
    >
      <SlotHost
        name="sidebar"
        fallback={
          <section className="px-2 py-3">
            <h2 className="px-2.5 pb-1 text-caption tracking-[0.02em] text-muted uppercase">
              Agentes
            </h2>
            <p className="px-2.5 py-1.5 text-caption text-muted">Nenhuma equipe aberta.</p>
          </section>
        }
      />
    </aside>
  );
}

function Inspector({ width }: { width: number }) {
  return (
    <aside
      aria-label="Inspetor"
      style={{ width }}
      className="flex shrink-0 flex-col border-l border-subtle bg-surface"
    >
      <div className="border-b border-subtle px-3 py-2">
        <h2 className="text-caption tracking-[0.02em] text-muted uppercase">Inspetor</h2>
      </div>
      <SlotHost
        name="inspector"
        fallback={
          <p className="px-3 py-2 text-caption text-muted">
            Selecione um agente para ver os detalhes.
          </p>
        }
      />
    </aside>
  );
}
