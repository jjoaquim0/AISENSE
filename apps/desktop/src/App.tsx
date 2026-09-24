import { Users } from 'lucide-react';
import { lazy, Suspense, useEffect } from 'react';
import { EmptyState, TooltipProvider } from '@/components/ui';
import { FlowBench } from '@/features/dev/FlowBench';
import { GridBench } from '@/features/dev/GridBench';
import { KitchenSink } from '@/features/dev/KitchenSink';
import { Onboarding } from '@/features/onboarding/Onboarding';
import { useSettings } from '@/features/settings/store';
import { AppShell } from '@/features/shell/AppShell';
import { useNav } from '@/features/shell/nav';
import { TeamsHome } from '@/features/teams/TeamsHome';
import { isDesktop } from '@/lib/api';

// Sob demanda: o preview de Markdown do editor não entra no carregamento inicial.
const SettingsScreen = lazy(() =>
  import('@/features/settings/SettingsScreen').then((m) => ({ default: m.SettingsScreen })),
);
const SkillLibraryScreen = lazy(() =>
  import('@/features/skills/SkillLibraryScreen').then((m) => ({ default: m.SkillLibraryScreen })),
);

export function App() {
  const screen = useNav((s) => s.screen);
  const loadSettings = useSettings((s) => s.load);
  const settingsLoaded = useSettings((s) => s.view !== null || s.error !== null);
  const onboarding = useSettings((s) => s.view !== null && !s.view.settings.onboardingDone);
  useEffect(() => {
    if (isDesktop()) void loadSettings();
  }, [loadSettings]);
  // Amostra do design system, só em desenvolvimento (F00-03 / F00-06).
  if (import.meta.env.DEV && window.location.hash.startsWith('#/dev/grid')) {
    return <GridBench />;
  }
  if (import.meta.env.DEV && window.location.hash.startsWith('#/dev/flow')) {
    return <FlowBench />;
  }
  if (import.meta.env.DEV && window.location.hash === '#/dev') {
    return (
      <TooltipProvider delayDuration={300}>
        <KitchenSink />
      </TooltipProvider>
    );
  }

  if (isDesktop() && !settingsLoaded) {
    // Uma leitura de arquivo local: esperar evita piscar o shell antes do onboarding.
    return <div className="h-full bg-base" aria-busy="true" />;
  }
  if (onboarding) {
    return (
      <TooltipProvider delayDuration={300}>
        <Onboarding />
      </TooltipProvider>
    );
  }

  return (
    <TooltipProvider delayDuration={300}>
      <AppShell>
        {isDesktop() ? (
          <>
            {/* A sala da equipe continua montada: terminais e seleção sobrevivem à ida às skills. */}
            <div hidden={screen !== 'teams'} className="h-full">
              <TeamsHome />
            </div>
            {screen === 'skills' && (
              <Suspense fallback={null}>
                <SkillLibraryScreen />
              </Suspense>
            )}
            {screen === 'settings' && (
              <Suspense fallback={null}>
                <SettingsScreen />
              </Suspense>
            )}
          </>
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
