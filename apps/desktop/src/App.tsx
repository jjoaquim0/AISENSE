import { useEffect, useState } from 'react';
import { AppShell, EmptyWorkspace } from '@/features/shell/AppShell';
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

  return (
    <AppShell>
      <EmptyWorkspace version={version} />
    </AppShell>
  );
}
