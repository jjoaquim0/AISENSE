import { Users } from 'lucide-react';
import { EmptyState, TooltipProvider } from '@/components/ui';
import { KitchenSink } from '@/features/dev/KitchenSink';
import { AppShell } from '@/features/shell/AppShell';
import { TeamsHome } from '@/features/teams/TeamsHome';
import { isDesktop } from '@/lib/api';

export function App() {
  // Amostra do design system, só em desenvolvimento (F00-03 / F00-06).
  if (import.meta.env.DEV && window.location.hash === '#/dev') {
    return (
      <TooltipProvider delayDuration={300}>
        <KitchenSink />
      </TooltipProvider>
    );
  }

  return (
    <TooltipProvider delayDuration={300}>
      <AppShell>
        {isDesktop() ? (
          <TeamsHome />
        ) : (
          // No `vite dev` puro (fora da janela do Tauri) não existe core para responder.
          <EmptyState
            icon={<Users size={22} />}
            title="Abra pelo aplicativo"
            description="As equipes vivem no core do AISENSE. Rode `pnpm app` para abrir a janela completa; `#/dev` mostra o design system."
          />
        )}
      </AppShell>
    </TooltipProvider>
  );
}
