import { Moon, Plus, Sun, Users } from 'lucide-react';
import type { ReactNode } from 'react';
import { StatusDot } from '@/components/ui/StatusDot';
import { cn } from '@/lib/cn';
import { isDark, useTheme } from '@/lib/theme';

/**
 * Estrutura global da janela — docs/09-telas-e-fluxos.md:
 * trilho de equipes (48px) · sidebar (240px) · área principal · inspetor (320px).
 */
export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full flex-col bg-base text-primary">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <TeamRail />
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-auto">{children}</main>
      </div>
    </div>
  );
}

function TitleBar() {
  const { theme, toggle } = useTheme();
  return (
    <header
      data-tauri-drag-region
      className="flex h-10 shrink-0 items-center justify-between border-b border-subtle bg-surface pr-2 pl-20"
    >
      <span className="text-label text-secondary">AISENSE</span>
      <button
        type="button"
        onClick={toggle}
        aria-label={isDark(theme) ? 'Usar tema claro' : 'Usar tema escuro'}
        className="flex size-7 items-center justify-center rounded-md text-secondary transition-colors duration-100 hover:bg-hover hover:text-primary"
      >
        {isDark(theme) ? <Sun size={15} /> : <Moon size={15} />}
      </button>
    </header>
  );
}

function TeamRail() {
  return (
    <nav
      aria-label="Equipes"
      className="flex w-12 shrink-0 flex-col items-center gap-2 border-r border-subtle bg-surface py-3"
    >
      <button
        type="button"
        aria-label="Nova equipe"
        className="flex size-8 items-center justify-center rounded-lg border border-dashed border-strong text-muted transition-colors duration-100 hover:border-accent hover:text-accent"
      >
        <Plus size={16} />
      </button>
    </nav>
  );
}

function Sidebar() {
  return (
    <aside
      aria-label="Agentes da equipe"
      className="flex w-60 shrink-0 flex-col border-r border-subtle bg-surface"
    >
      <SidebarSection title="Agentes">
        <p className="px-2.5 py-1.5 text-caption text-muted">Nenhuma equipe selecionada.</p>
      </SidebarSection>
      <div className="mt-auto border-t border-subtle px-2.5 py-2">
        <StatusDot state="stopped" withLabel />
      </div>
    </aside>
  );
}

function SidebarSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="px-2 py-3">
      <h2 className={cn('px-2.5 pb-1 text-caption tracking-[0.02em] text-muted uppercase')}>
        {title}
      </h2>
      {children}
    </section>
  );
}

export function EmptyWorkspace({ version }: { version: string | null }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      <div className="flex size-12 items-center justify-center rounded-xl border border-subtle bg-surface text-muted">
        <Users size={22} />
      </div>
      <h1 className="text-display">Monte sua primeira equipe</h1>
      <p className="max-w-sm text-body text-secondary">
        Uma equipe reúne agentes em terminais reais que conversam entre si e compartilham um quadro
        de trabalho.
      </p>
      <p className="text-caption text-muted">
        {version ? `Fundação · v${version}` : 'Fase 00 — Fundação'}
      </p>
    </div>
  );
}
