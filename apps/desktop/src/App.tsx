import { Users } from 'lucide-react';
import { useEffect, useState } from 'react';
import { EmptyState, TooltipProvider } from '@/components/ui';
import { KitchenSink } from '@/features/dev/KitchenSink';
import { AppShell } from '@/features/shell/AppShell';
import { api, isDesktop } from '@/lib/api';

export function App() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    // No `vite dev` puro (fora da janela do Tauri) não existe core para responder.
    if (!isDesktop()) return;
    let active = true;
    api
      .appInfo()
      .then((info) => {
        if (active) setVersion(info.version);
      })
      .catch((error: unknown) => {
        console.error('Falha ao consultar o core:', error);
      });
    return () => {
      active = false;
    };
  }, []);

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
        <EmptyState
          icon={<Users size={22} />}
          title="Monte sua primeira equipe"
          description="Uma equipe reúne agentes em terminais reais que conversam entre si e compartilham um quadro de trabalho."
          note={version ? `Fundação · v${version}` : 'Fase 00 — Fundação'}
        />
      </AppShell>
    </TooltipProvider>
  );
}
