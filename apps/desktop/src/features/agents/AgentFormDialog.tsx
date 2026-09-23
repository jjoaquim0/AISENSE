import { zodResolver } from '@hookform/resolvers/zod';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { AlertTriangle, Check, ChevronRight, FolderOpen, Minus, X } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { type Resolver, useForm } from 'react-hook-form';
import { Button, Dialog, Input } from '@/components/ui';
import { runtimesApi } from '@/features/runtimes/api';
import { errorMessage } from '@/features/teams/api';
import { cn } from '@/lib/cn';
import type { Agent } from '@/types/generated/Agent';
import type { RuntimeInfo } from '@/types/generated/RuntimeInfo';
import type { TeamId } from '@/types/generated/TeamId';
import {
  type AgentFormValues,
  agentSchema,
  CUSTOM_ADAPTER,
  draftFromForm,
  EMPTY_FORM,
  formFromAgent,
} from './agentForm';
import { agentsApi } from './api';

const COLORS = ['violet', 'cyan', 'emerald', 'amber', 'rose', 'indigo', 'teal', 'fuchsia'] as const;

interface AgentFormDialogProps {
  teamId: TeamId;
  /** Presente = edição. */
  agent?: Agent;
  /** Todos os agentes da equipe, para validar o endereço antes de submeter. */
  siblings: Agent[];
  running: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** `restartRequired` quando a edição de um agente vivo só vale no próximo início. */
  onSaved: (agent: Agent, restartRequired: boolean) => void;
}

/** T5 — Criar/editar agente (docs/09). */
export function AgentFormDialog({
  teamId,
  agent,
  siblings,
  running,
  open,
  onOpenChange,
  onSaved,
}: AgentFormDialogProps) {
  const editing = agent !== undefined;
  const otherHandles = useMemo(
    () => siblings.filter((s) => s.id !== agent?.id).map((s) => s.handle),
    [siblings, agent],
  );
  const {
    register,
    handleSubmit,
    watch,
    setValue,
    reset,
    formState: { errors, isSubmitting, dirtyFields },
  } = useForm<AgentFormValues>({
    // O zod v4 infere tipos de entrada e saída iguais aqui; o cast só alinha as assinaturas.
    resolver: zodResolver(agentSchema(otherHandles)) as Resolver<AgentFormValues>,
    defaultValues: agent ? formFromAgent(agent) : EMPTY_FORM,
    mode: 'onChange',
  });
  const [runtimes, setRuntimes] = useState<RuntimeInfo[] | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    reset(agent ? formFromAgent(agent) : EMPTY_FORM);
    setError(null);
    runtimesApi
      .overview()
      .then((o) => setRuntimes(o.runtimes))
      .catch((e: unknown) => setError(errorMessage(e)));
  }, [open, agent, reset]);

  // O endereço acompanha o nome até alguém editá-lo à mão (T5: "gerado do nome, editável").
  const name = watch('name');
  useEffect(() => {
    if (editing || dirtyFields.handle || !name.trim()) return;
    const timer = setTimeout(() => {
      void agentsApi.suggestHandle(name).then((handle) => {
        if (handle) setValue('handle', handle, { shouldValidate: true });
      });
    }, 150);
    return () => clearTimeout(timer);
  }, [name, editing, dirtyFields.handle, setValue]);

  const adapterId = watch('adapterId');
  const color = watch('color');
  const selected = runtimes?.find((r) => r.adapter.id === adapterId);
  const supportsModel = Boolean(selected?.adapter.capabilities.modelFlag);

  const submit = handleSubmit(async (values) => {
    setError(null);
    const draft = draftFromForm(values);
    try {
      if (agent) {
        const update = await agentsApi.update(agent.id, draft);
        onSaved(update.agent, update.restartRequired);
      } else {
        onSaved(await agentsApi.create(teamId, draft), false);
      }
      onOpenChange(false);
    } catch (e: unknown) {
      setError(errorMessage(e));
    }
  });

  const pickFolder = async () => {
    const folder = await openDialog({ directory: true, title: 'Diretório do agente' });
    if (typeof folder === 'string') setValue('workdir', folder, { shouldDirty: true });
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="lg"
      title={editing ? `Editar @${agent.handle}` : 'Novo agente'}
      description={
        editing
          ? 'As mudanças são gravadas na hora.'
          : 'Um agente é um terminal com um runtime de IA e um papel na equipe.'
      }
      footer={
        <>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancelar
          </Button>
          <Button variant="primary" disabled={isSubmitting} onClick={() => void submit()}>
            {editing ? 'Salvar' : 'Criar'}
          </Button>
        </>
      }
    >
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        {editing && running && (
          <p className="flex items-start gap-1.5 rounded-md border border-subtle bg-surface px-2.5 py-2 text-caption text-secondary">
            <AlertTriangle size={13} className="mt-0.5 shrink-0 text-awaiting" />
            Este agente está rodando. Runtime, papel, ambiente e argumentos só mudam depois de
            reiniciá-lo.
          </p>
        )}

        <div className="grid grid-cols-2 gap-3">
          <Input label="Nome" {...register('name')} error={errors.name?.message} autoFocus />
          <Input
            label="Endereço"
            {...register('handle')}
            error={errors.handle?.message}
            hint="Como os outros agentes o chamam: @endereço"
            className="font-mono"
          />
        </div>

        <div className="flex flex-col gap-1">
          <label htmlFor="agent-role" className="text-label text-secondary">
            Papel
          </label>
          <textarea
            id="agent-role"
            rows={3}
            {...register('role')}
            placeholder="Implementa e mantém a API. Avisa o @frontend quando um contrato muda."
            className="rounded-md border border-strong bg-surface px-2.5 py-1.5 text-body text-primary placeholder:text-muted focus:border-accent"
          />
          <p className="text-caption text-muted">Vai no prompt inicial do agente.</p>
        </div>

        <fieldset className="flex flex-col gap-1">
          <legend className="text-label text-secondary">Runtime</legend>
          {!runtimes && <p className="text-caption text-muted">Verificando os runtimes…</p>}
          <div className="divide-y divide-subtle rounded-lg border border-subtle">
            {runtimes?.map((r) => (
              <RuntimeOption
                key={r.adapter.id}
                runtime={r}
                checked={adapterId === r.adapter.id}
                inputProps={register('adapterId')}
              />
            ))}
          </div>
        </fieldset>

        {adapterId === CUSTOM_ADAPTER && (
          <Input
            label="Comando"
            {...register('command')}
            error={errors.command?.message}
            placeholder="htop"
            className="font-mono"
          />
        )}
        {supportsModel && (
          <Input
            label="Modelo"
            {...register('model')}
            hint="Vazio usa o padrão do runtime."
            placeholder="opus"
          />
        )}

        <div className="flex items-end gap-2">
          <div className="flex-1">
            <Input
              label="Diretório"
              {...register('workdir')}
              placeholder="Herdar da equipe"
              className="font-mono"
            />
          </div>
          <Button onClick={() => void pickFolder()}>
            <FolderOpen size={14} /> Escolher…
          </Button>
        </div>

        <fieldset className="flex flex-col gap-1">
          <legend className="text-label text-secondary">Cor</legend>
          <div className="flex gap-1.5 pt-1">
            {COLORS.map((c) => (
              <button
                key={c}
                type="button"
                aria-label={c}
                aria-pressed={color === c}
                onClick={() => setValue('color', c, { shouldDirty: true })}
                className={cn(
                  'flex size-6 items-center justify-center rounded-full text-accent-fg',
                  color === c && 'ring-2 ring-ring ring-offset-2 ring-offset-raised',
                )}
                style={{ background: `var(--agent-${c})` }}
              >
                {color === c && <Check size={12} />}
              </button>
            ))}
          </div>
        </fieldset>

        <button
          type="button"
          onClick={() => setAdvanced(!advanced)}
          aria-expanded={advanced}
          className="flex items-center gap-1 self-start text-label text-secondary hover:text-primary"
        >
          <ChevronRight size={14} className={cn('transition-transform', advanced && 'rotate-90')} />
          Avançado
        </button>
        {advanced && (
          <div className="flex flex-col gap-3 border-l border-subtle pl-3">
            <Choice
              legend="Receber mensagens"
              options={[
                ['pull', 'Caixa de entrada'],
                ['push', 'Injetar no terminal'],
                ['hook', 'Hook do runtime'],
              ]}
              inputProps={register('deliveryMode')}
            />
            <label className="flex items-center gap-2 text-body text-primary">
              <input type="checkbox" {...register('autostart')} />
              Iniciar com a equipe
            </label>
            <label className="flex items-center gap-2 text-body text-primary">
              Reiniciar em caso de falha
              <select
                {...register('restartPolicy')}
                className="h-7 rounded-md border border-strong bg-surface px-2 text-label text-primary"
              >
                <option value="on-crash">só se cair com erro</option>
                <option value="always">sempre</option>
                <option value="never">nunca</option>
              </select>
            </label>
            <Choice
              legend="Autonomia"
              options={[
                ['ask', 'Perguntar'],
                ['trusted', 'Confiar'],
              ]}
              inputProps={register('autonomy')}
            />
            <label className="flex items-center gap-2 text-body text-primary">
              Bancada
              <select
                {...register('workbench')}
                className="h-7 rounded-md border border-strong bg-surface px-2 text-label text-primary"
              >
                <option value="inherit">seguir a equipe</option>
                <option value="own">própria (git worktree)</option>
                <option value="shared">diretório compartilhado</option>
              </select>
            </label>
            <TextBlock
              id="agent-env"
              label="Variáveis de ambiente"
              hint="Uma por linha: CHAVE=valor"
              error={errors.envText?.message}
              inputProps={register('envText')}
            />
            <TextBlock
              id="agent-args"
              label="Argumentos extras"
              hint="Um por linha, passados ao runtime depois dos do adaptador."
              inputProps={register('argsText')}
            />
          </div>
        )}

        {error && (
          <p className="text-caption text-failed" role="alert">
            {error}
          </p>
        )}
        {/* Enter no formulário submete. */}
        <button type="submit" className="hidden" aria-hidden tabIndex={-1} />
      </form>
    </Dialog>
  );
}

type Registered = ReturnType<ReturnType<typeof useForm<AgentFormValues>>['register']>;

function RuntimeOption({
  runtime,
  checked,
  inputProps,
}: {
  runtime: RuntimeInfo;
  checked: boolean;
  inputProps: Registered;
}) {
  const { adapter, status } = runtime;
  const detail =
    status.status === 'available'
      ? (status.version ?? 'instalado')
      : status.status === 'missing'
        ? 'não instalado'
        : 'comando definido aqui';
  const icon =
    status.status === 'available' ? (
      <Check size={12} className="text-idle" />
    ) : status.status === 'missing' ? (
      <X size={12} className="text-failed" />
    ) : (
      <Minus size={12} className="text-muted" />
    );
  return (
    <label
      className={cn(
        'flex cursor-pointer items-center justify-between gap-3 px-3 py-1.5',
        checked && 'bg-hover',
      )}
    >
      <span className="flex items-center gap-2 text-body text-primary">
        <input type="radio" value={adapter.id} {...inputProps} />
        {adapter.name}
      </span>
      <span className="flex min-w-0 items-center gap-1 text-caption text-muted">
        {icon}
        <span className="truncate">{detail}</span>
      </span>
    </label>
  );
}

function Choice({
  legend,
  options,
  inputProps,
}: {
  legend: string;
  options: [string, string][];
  inputProps: Registered;
}) {
  return (
    <fieldset className="flex flex-wrap items-center gap-x-3 gap-y-1">
      <legend className="mb-1 text-label text-secondary">{legend}</legend>
      {options.map(([id, label]) => (
        <label key={id} className="flex items-center gap-1.5 text-body text-primary">
          <input type="radio" value={id} {...inputProps} />
          {label}
        </label>
      ))}
    </fieldset>
  );
}

function TextBlock({
  id,
  label,
  hint,
  error,
  inputProps,
}: {
  id: string;
  label: string;
  hint: string;
  error?: string;
  inputProps: Registered;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="text-label text-secondary">
        {label}
      </label>
      <textarea
        id={id}
        rows={3}
        {...inputProps}
        aria-invalid={error ? true : undefined}
        className={cn(
          'rounded-md border bg-surface px-2.5 py-1.5 font-mono text-label text-primary',
          error ? 'border-failed' : 'border-strong focus:border-accent',
        )}
      />
      <p className={cn('text-caption', error ? 'text-failed' : 'text-muted')}>{error ?? hint}</p>
    </div>
  );
}
