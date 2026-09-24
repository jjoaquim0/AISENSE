import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { AlertTriangle, FolderOpen, Monitor, Moon, Sun } from 'lucide-react';
import { type ReactNode, useEffect, useState } from 'react';
import { Logo } from '@/components/brand/Logo';
import { Button, Input } from '@/components/ui';
import { runtimesApi } from '@/features/runtimes/api';
import { RuntimeList } from '@/features/runtimes/RuntimeList';
import { settingsApi } from '@/features/settings/api';
import { useSettings } from '@/features/settings/store';
import { describeStartReport, errorMessage, teamsApi } from '@/features/teams/api';
import { TEMPLATES } from '@/features/teams/CreateTeamWizard';
import { useTeams } from '@/features/teams/store';
import { cn } from '@/lib/cn';
import type { PlannedAgent } from '@/types/generated/PlannedAgent';
import type { RuntimeInfo } from '@/types/generated/RuntimeInfo';
import type { TeamTemplate } from '@/types/generated/TeamTemplate';
import type { ThemePreference } from '@/types/generated/ThemePreference';
import { withRunnableRuntimes } from './plan';

type Step = 1 | 2 | 3;

/** Modelos que fazem sentido para a primeira equipe: pequenos e de desenvolvimento. */
const FIRST_TEMPLATES: TeamTemplate[] = ['duo-dev', 'full-squad', 'empty'];

/**
 * T1 — Primeira execução (docs/09; F08-04). Três passos, pulável, nunca mais aparece:
 * o que está instalado, o tema e a primeira equipe — criada e já iniciada, para o
 * usuário sair daqui com terminais vivos.
 */
export function Onboarding() {
  const [step, setStep] = useState<Step>(1);
  const received = useSettings((s) => s.received);
  const [leaving, setLeaving] = useState(false);

  const finish = async () => {
    setLeaving(true);
    try {
      received(await settingsApi.onboardingDone());
    } catch {
      // Sem gravar, o onboarding volta na próxima abertura; hoje o usuário segue.
      const view = useSettings.getState().view;
      if (view) received({ ...view.settings, onboardingDone: true });
    }
  };

  return (
    <div className="flex h-full items-center justify-center overflow-auto bg-base p-6 text-primary">
      <main
        aria-labelledby="onboarding-title"
        className="flex w-full max-w-lg flex-col gap-5 rounded-xl border border-subtle bg-surface p-6"
      >
        <header className="flex flex-col items-center gap-1 text-center">
          {step === 1 && <Logo size={30} className="mb-3 text-primary" />}
          <p className="text-caption text-muted" aria-live="polite">
            Passo {step} de 3
          </p>
          <h1 id="onboarding-title" className="text-title text-primary">
            {step === 1
              ? 'Bem-vindo ao aisense'
              : step === 2
                ? 'Escolha o tema'
                : 'Sua primeira equipe'}
          </h1>
          <p className="text-body text-secondary">
            {step === 1
              ? 'Monte equipes de IA que trabalham juntas.'
              : step === 2
                ? 'Dá para trocar depois com ⌘⇧D ou nas Configurações.'
                : 'Crie e já inicie — os terminais sobem em seguida.'}
          </p>
        </header>

        {step === 1 && <RuntimeList />}
        {step === 2 && <ThemeStep />}
        {step === 3 && <FirstTeamStep onDone={() => void finish()} onBack={() => setStep(2)} />}

        {step < 3 && (
          <footer className="flex justify-between gap-2">
            {step === 1 ? (
              <Button variant="ghost" onClick={() => void finish()} disabled={leaving}>
                Pular
              </Button>
            ) : (
              <Button variant="ghost" onClick={() => setStep(1)}>
                Voltar
              </Button>
            )}
            <Button variant="primary" onClick={() => setStep((step + 1) as Step)} autoFocus>
              Continuar →
            </Button>
          </footer>
        )}
        {step === 3 && (
          <Button variant="ghost" size="sm" onClick={() => void finish()} disabled={leaving}>
            Pular e criar depois
          </Button>
        )}
      </main>
    </div>
  );
}

function ThemeStep() {
  const theme = useSettings((s) => s.view?.settings.appearance.theme ?? 'system');
  const update = useSettings((s) => s.update);
  const options: [ThemePreference, string, ReactNode][] = [
    ['light', 'Claro', <Sun key="l" size={18} />],
    ['dark', 'Escuro', <Moon key="d" size={18} />],
    ['system', 'Sistema', <Monitor key="s" size={18} />],
  ];
  return (
    <fieldset className="grid grid-cols-3 gap-2">
      <legend className="sr-only">Tema</legend>
      {options.map(([id, label, icon]) => (
        <label
          key={id}
          className={cn(
            'flex cursor-pointer flex-col items-center gap-2 rounded-lg border p-4 text-body',
            'transition-colors duration-100 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-ring',
            theme === id
              ? 'border-emphasis text-primary'
              : 'border-subtle text-secondary hover:bg-hover',
          )}
        >
          <input
            type="radio"
            name="onboarding-theme"
            value={id}
            checked={theme === id}
            onChange={() =>
              void update((d) => {
                d.appearance.theme = id;
              })
            }
            className="sr-only"
          />
          {icon}
          {label}
        </label>
      ))}
    </fieldset>
  );
}

function FirstTeamStep({ onDone, onBack }: { onDone: () => void; onBack: () => void }) {
  const [name, setName] = useState('Minha equipe');
  const [workdir, setWorkdir] = useState('');
  const [template, setTemplate] = useState<TeamTemplate>('duo-dev');
  const [startNow, setStartNow] = useState(true);
  const [planned, setPlanned] = useState<PlannedAgent[] | null>(null);
  const [runtimes, setRuntimes] = useState<RuntimeInfo[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { load, selectTeam } = useTeams();

  useEffect(() => {
    let active = true;
    setPlanned(null);
    Promise.all([teamsApi.planTemplate(template), runtimesApi.overview()])
      .then(([plan, overview]) => {
        if (!active) return;
        setRuntimes(overview.runtimes);
        setPlanned(withRunnableRuntimes(plan, overview.runtimes));
      })
      .catch((e: unknown) => active && setError(errorMessage(e)));
    return () => {
      active = false;
    };
  }, [template]);

  const pickFolder = async () => {
    try {
      const folder = await openDialog({ directory: true, title: 'Pasta de trabalho da equipe' });
      if (typeof folder === 'string') setWorkdir(folder);
    } catch (e: unknown) {
      setError(errorMessage(e));
    }
  };

  const create = async () => {
    if (!planned) return;
    setBusy(true);
    setError(null);
    try {
      const team = await teamsApi.create(
        {
          name: name.trim(),
          mission: '',
          workdir: workdir.trim(),
          color: 'violet',
          workspaceMode: 'shared',
        },
        planned.map((p) => p.draft),
      );
      await load();
      selectTeam(team.id);
      onDone();
      if (startNow && planned.length > 0) {
        // Depois de sair do onboarding: o progresso aparece na própria Sala da Equipe.
        const report = await teamsApi.start(team.id);
        const lines = describeStartReport(report, (id) => {
          const agent = useTeams
            .getState()
            .teams?.find((t) => t.team.id === team.id)
            ?.agents.find((a) => a.id === id);
          return agent?.handle ?? id;
        });
        if (lines.length > 0) console.warn('Início da primeira equipe com ressalvas:', lines);
      }
    } catch (e: unknown) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };

  const nameOf = (id: string) => runtimes.find((r) => r.adapter.id === id)?.adapter.name ?? id;
  const ready = name.trim() !== '' && workdir.trim() !== '' && planned !== null;

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (ready && !busy) void create();
      }}
    >
      <Input label="Nome" value={name} onChange={(e) => setName(e.target.value)} maxLength={64} />
      <div className="flex items-end gap-2">
        <div className="flex-1">
          <Input
            label="Pasta do projeto"
            value={workdir}
            onChange={(e) => setWorkdir(e.target.value)}
            placeholder="/home/voce/projetos/api"
            hint="Onde os agentes vão trabalhar."
            className="font-mono"
            autoFocus
          />
        </div>
        <Button onClick={() => void pickFolder()} className="mb-5">
          <FolderOpen size={14} /> Escolher…
        </Button>
      </div>
      <fieldset className="flex flex-col gap-1">
        <legend className="mb-1 text-label text-secondary">Modelo</legend>
        {TEMPLATES.filter((t) => FIRST_TEMPLATES.includes(t.id)).map((t) => (
          <label key={t.id} className="flex cursor-pointer items-start gap-2 py-0.5">
            <input
              type="radio"
              name="onboarding-template"
              checked={template === t.id}
              onChange={() => setTemplate(t.id)}
              className="mt-1"
            />
            <span className="flex flex-col">
              <span className="text-body text-primary">{t.title}</span>
              <span className="text-caption text-muted">{t.agents}</span>
            </span>
          </label>
        ))}
      </fieldset>
      {planned?.some((p) => p.draft.adapterId !== p.preferredAdapterId) && (
        <p className="flex gap-1.5 text-caption text-secondary">
          <AlertTriangle size={13} className="mt-0.5 shrink-0 text-awaiting" />
          <span>
            {planned
              .filter((p) => p.draft.adapterId !== p.preferredAdapterId)
              .map(
                (p) =>
                  `@${p.draft.handle} usará ${nameOf(p.draft.adapterId)} (${nameOf(p.preferredAdapterId)} não está instalado)`,
              )
              .join('; ')}
            . Dá para trocar depois, no agente.
          </span>
        </p>
      )}
      <label className="flex items-center gap-2 text-body text-primary">
        <input type="checkbox" checked={startNow} onChange={(e) => setStartNow(e.target.checked)} />
        Iniciar os agentes ao criar
      </label>
      {error && (
        <p role="alert" className="text-caption text-failed">
          {error}
        </p>
      )}
      <div className="flex justify-between gap-2">
        <Button variant="ghost" onClick={onBack} disabled={busy}>
          Voltar
        </Button>
        <Button type="submit" variant="primary" disabled={!ready || busy}>
          {busy ? 'Criando…' : 'Criar equipe →'}
        </Button>
      </div>
    </form>
  );
}
