import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { AlertTriangle, Check, FolderOpen } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Button, Dialog, Input } from '@/components/ui';
import { runtimesApi } from '@/features/runtimes/api';
import { cn } from '@/lib/cn';
import type { AgentColor } from '@/types/generated/AgentColor';
import type { PlannedAgent } from '@/types/generated/PlannedAgent';
import type { RuntimeInfo } from '@/types/generated/RuntimeInfo';
import type { TeamDraft } from '@/types/generated/TeamDraft';
import type { TeamTemplate } from '@/types/generated/TeamTemplate';
import { errorMessage, teamsApi } from './api';
import { useTeams } from './store';

const COLORS: AgentColor[] = [
  'violet',
  'cyan',
  'emerald',
  'amber',
  'rose',
  'indigo',
  'teal',
  'fuchsia',
];

const TEMPLATES: { id: TeamTemplate; title: string; agents: string }[] = [
  { id: 'empty', title: 'Vazio', agents: 'Comece sem agentes e adicione depois.' },
  { id: 'duo-dev', title: 'Dupla Dev', agents: '@dev implementa, @revisor revisa.' },
  {
    id: 'full-squad',
    title: 'Squad completo',
    agents: '@arquiteto, @backend, @frontend e @revisor.',
  },
  {
    id: 'research',
    title: 'Pesquisa',
    agents: '@coordenador, três pesquisadores e @sintetizador.',
  },
  {
    id: 'operations',
    title: 'Operação',
    agents: '@monitor (shell, longa duração) e @triagem.',
  },
];

const EMPTY_DRAFT: TeamDraft = {
  name: '',
  mission: '',
  workdir: '',
  color: 'violet',
  workspaceMode: 'shared',
};

type Step = 1 | 2 | 3;

/** T3 — Criar equipe, em três passos (docs/09). */
export function CreateTeamWizard() {
  const { wizardOpen, setWizardOpen, load } = useTeams();
  const [step, setStep] = useState<Step>(1);
  const [draft, setDraft] = useState<TeamDraft>(EMPTY_DRAFT);
  const [template, setTemplate] = useState<TeamTemplate>('duo-dev');
  const [planned, setPlanned] = useState<PlannedAgent[] | null>(null);
  const [runtimes, setRuntimes] = useState<RuntimeInfo[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reset = () => {
    setStep(1);
    setDraft(EMPTY_DRAFT);
    setTemplate('duo-dev');
    setPlanned(null);
    setError(null);
  };

  const close = (open: boolean) => {
    setWizardOpen(open);
    if (!open) reset();
  };

  // Passo 3: planeja o modelo contra os runtimes instalados nesta máquina.
  useEffect(() => {
    if (step !== 3) return;
    let active = true;
    setPlanned(null);
    Promise.all([teamsApi.planTemplate(template), runtimesApi.overview()])
      .then(([plan, overview]) => {
        if (!active) return;
        setPlanned(plan);
        setRuntimes(overview.runtimes);
      })
      .catch((e: unknown) => active && setError(errorMessage(e)));
    return () => {
      active = false;
    };
  }, [step, template]);

  const create = async () => {
    if (!planned) return;
    setBusy(true);
    setError(null);
    try {
      await teamsApi.create(
        draft,
        planned.map((p) => p.draft),
      );
      close(false);
      await load();
    } catch (e: unknown) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const identityReady = draft.name.trim() !== '' && draft.workdir.trim() !== '';

  return (
    <Dialog
      open={wizardOpen}
      onOpenChange={close}
      size="lg"
      title="Nova equipe"
      description={
        step === 1
          ? 'Passo 1 de 3 — quem é a equipe e onde ela trabalha.'
          : step === 2
            ? 'Passo 2 de 3 — comece de um modelo para não montar do zero.'
            : 'Passo 3 de 3 — revise os agentes antes de criar.'
      }
      footer={
        <>
          {step > 1 && (
            <Button variant="ghost" onClick={() => setStep((step - 1) as Step)} disabled={busy}>
              Voltar
            </Button>
          )}
          {step < 3 ? (
            <Button
              variant="primary"
              disabled={step === 1 && !identityReady}
              onClick={() => setStep((step + 1) as Step)}
            >
              Continuar
            </Button>
          ) : (
            <Button variant="primary" disabled={!planned || busy} onClick={() => void create()}>
              Criar equipe
            </Button>
          )}
        </>
      }
    >
      {step === 1 && <IdentityStep draft={draft} onChange={setDraft} />}
      {step === 2 && <TemplateStep value={template} onChange={setTemplate} />}
      {step === 3 && (
        <ReviewStep
          planned={planned}
          runtimes={runtimes}
          onChange={setPlanned}
          templateTitle={TEMPLATES.find((t) => t.id === template)?.title ?? ''}
        />
      )}
      {error && (
        <p className="mt-3 text-caption text-failed" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}

function IdentityStep({
  draft,
  onChange,
}: {
  draft: TeamDraft;
  onChange: (draft: TeamDraft) => void;
}) {
  const pickFolder = async () => {
    const folder = await openDialog({ directory: true, title: 'Diretório de trabalho da equipe' });
    if (typeof folder === 'string') onChange({ ...draft, workdir: folder });
  };

  return (
    <div className="flex flex-col gap-3">
      <Input
        label="Nome"
        value={draft.name}
        onChange={(e) => onChange({ ...draft, name: e.target.value })}
        placeholder="Squad Produto"
        maxLength={64}
        autoFocus
      />
      <div className="flex flex-col gap-1">
        <label htmlFor="team-mission" className="text-label text-secondary">
          Missão
        </label>
        <textarea
          id="team-mission"
          value={draft.mission}
          onChange={(e) => onChange({ ...draft, mission: e.target.value })}
          rows={3}
          placeholder="Migrar a autenticação para OAuth sem derrubar o login atual."
          className="rounded-md border border-strong bg-surface px-2.5 py-1.5 text-body text-primary placeholder:text-muted focus:border-accent"
        />
        <p className="text-caption text-muted">Isto vai no prompt de todos os agentes.</p>
      </div>
      <div className="flex items-end gap-2">
        <div className="flex-1">
          <Input
            label="Diretório de trabalho"
            value={draft.workdir}
            onChange={(e) => onChange({ ...draft, workdir: e.target.value })}
            placeholder="C:\\projetos\\api"
            className="font-mono"
          />
        </div>
        <Button onClick={() => void pickFolder()}>
          <FolderOpen size={14} /> Escolher…
        </Button>
      </div>
      <fieldset className="flex flex-col gap-1">
        <legend className="text-label text-secondary">Onde os agentes trabalham</legend>
        {(
          [
            [
              'shared',
              'Diretório compartilhado',
              'Todos no mesmo checkout. Simples; bom para um agente escrevendo por vez.',
            ],
            [
              'per-agent',
              'Bancada por agente',
              'Cada agente num git worktree com branch próprio, para trabalharem em paralelo sem se atropelar.',
            ],
          ] as const
        ).map(([mode, title, hint]) => (
          <label key={mode} className="flex cursor-pointer items-start gap-2 py-0.5">
            <input
              type="radio"
              name="workspace-mode"
              checked={draft.workspaceMode === mode}
              onChange={() => onChange({ ...draft, workspaceMode: mode })}
              className="mt-1"
            />
            <span className="flex flex-col">
              <span className="text-body text-primary">{title}</span>
              <span className="text-caption text-muted">{hint}</span>
            </span>
          </label>
        ))}
      </fieldset>
      <fieldset className="flex flex-col gap-1">
        <legend className="text-label text-secondary">Cor</legend>
        <div className="flex gap-1.5 pt-1">
          {COLORS.map((color) => (
            <button
              key={color}
              type="button"
              aria-label={color}
              aria-pressed={draft.color === color}
              onClick={() => onChange({ ...draft, color })}
              className={cn(
                'flex size-6 items-center justify-center rounded-full text-accent-fg',
                draft.color === color && 'ring-2 ring-ring ring-offset-2 ring-offset-raised',
              )}
              style={{ background: `var(--agent-${color})` }}
            >
              {draft.color === color && <Check size={12} />}
            </button>
          ))}
        </div>
      </fieldset>
    </div>
  );
}

function TemplateStep({
  value,
  onChange,
}: {
  value: TeamTemplate;
  onChange: (template: TeamTemplate) => void;
}) {
  return (
    <fieldset className="grid grid-cols-2 gap-2">
      <legend className="sr-only">Modelo de equipe</legend>
      {TEMPLATES.map((t) => (
        <label
          key={t.id}
          className={cn(
            'flex cursor-pointer flex-col items-start gap-0.5 rounded-lg border p-3 transition-colors duration-100',
            'has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring',
            value === t.id ? 'border-accent bg-hover' : 'border-subtle hover:bg-hover',
          )}
        >
          <input
            type="radio"
            name="team-template"
            value={t.id}
            checked={value === t.id}
            onChange={() => onChange(t.id)}
            className="sr-only"
          />
          <span className="text-body text-primary">{t.title}</span>
          <span className="text-caption text-secondary">{t.agents}</span>
        </label>
      ))}
    </fieldset>
  );
}

function ReviewStep({
  planned,
  runtimes,
  onChange,
  templateTitle,
}: {
  planned: PlannedAgent[] | null;
  runtimes: RuntimeInfo[];
  onChange: (planned: PlannedAgent[]) => void;
  templateTitle: string;
}) {
  if (!planned) return <p className="text-caption text-muted">Verificando os runtimes…</p>;
  if (planned.length === 0) {
    return (
      <p className="text-body text-secondary">
        A equipe começa sem agentes. Você adiciona os agentes depois de criá-la.
      </p>
    );
  }

  const nameOf = (id: string) => runtimes.find((r) => r.adapter.id === id)?.adapter.name ?? id;
  const available = (id: string) =>
    runtimes.find((r) => r.adapter.id === id)?.status.status === 'available';
  // `custom` precisa de um comando por agente, que este passo não pede.
  const choices = runtimes.filter((r) => r.status.status !== 'perAgent');

  const setAdapter = (index: number, adapterId: string) =>
    onChange(planned.map((p, i) => (i === index ? { ...p, draft: { ...p.draft, adapterId } } : p)));

  return (
    <div className="flex flex-col gap-2">
      <p className="text-caption text-secondary">
        Modelo {templateTitle}: {planned.length} agentes. O runtime de cada um pode ser trocado.
      </p>
      <ul className="divide-y divide-subtle rounded-lg border border-subtle">
        {planned.map((p, index) => {
          const { draft } = p;
          const swapped = draft.adapterId !== p.preferredAdapterId;
          const missing = !available(draft.adapterId);
          return (
            <li key={draft.handle} className="flex flex-col gap-1 px-3 py-2">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <span className="text-body text-primary">@{draft.handle}</span>
                  <span className="ml-2 text-caption text-muted">{draft.name}</span>
                </div>
                <select
                  aria-label={`Runtime de @${draft.handle}`}
                  value={draft.adapterId}
                  onChange={(e) => setAdapter(index, e.target.value)}
                  className="h-7 rounded-md border border-strong bg-surface px-2 text-label text-primary"
                >
                  {choices.map((r) => (
                    <option key={r.adapter.id} value={r.adapter.id}>
                      {r.adapter.name}
                      {r.status.status === 'available' ? '' : ' (não instalado)'}
                    </option>
                  ))}
                </select>
              </div>
              {(swapped || missing) && (
                <p className="flex items-center gap-1.5 text-caption text-secondary">
                  <AlertTriangle size={12} className="shrink-0 text-awaiting" />
                  {missing
                    ? `${nameOf(draft.adapterId)} não está instalado — @${draft.handle} só vai subir depois de instalá-lo.`
                    : `${nameOf(p.preferredAdapterId)} não está instalado — @${draft.handle} usará ${nameOf(draft.adapterId)}.`}
                </p>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
