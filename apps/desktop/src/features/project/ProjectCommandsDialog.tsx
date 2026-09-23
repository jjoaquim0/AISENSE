import { AlertTriangle, FileCode } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Button, Dialog } from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import type { ProjectConfig } from '@/types/generated/ProjectConfig';
import type { ProjectLookup } from '@/types/generated/ProjectLookup';
import { projectApi } from './api';

interface ProjectCommandsDialogProps {
  workdir: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * Comandos do projeto (docs/17): o `aisense.toml` do diretório da equipe, o erro dele com
 * caminho e linha, ou a proposta detectada — que só vira arquivo quando o usuário aceita.
 */
export function ProjectCommandsDialog({ workdir, open, onOpenChange }: ProjectCommandsDialogProps) {
  const [lookup, setLookup] = useState<ProjectLookup | null>(null);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let active = true;
    setLookup(null);
    setError(null);
    projectApi
      .lookup(workdir)
      .then((result) => {
        if (!active) return;
        setLookup(result);
        if (result.status === 'missing' && result.proposal) setDraft(result.proposal.toml);
      })
      .catch((e: unknown) => active && setError(errorMessage(e)));
    return () => {
      active = false;
    };
  }, [open, workdir]);

  const accept = async () => {
    setBusy(true);
    setError(null);
    try {
      setLookup(await projectApi.accept(workdir, draft));
    } catch (e: unknown) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const proposing = lookup?.status === 'missing' && lookup.proposal !== null;

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="lg"
      title="Comandos do projeto"
      description="Como os agentes instalam, testam e constroem este projeto — definido uma vez, em aisense.toml."
      footer={
        proposing ? (
          <>
            <Button variant="ghost" onClick={() => onOpenChange(false)}>
              Agora não
            </Button>
            <Button variant="primary" disabled={busy} onClick={() => void accept()}>
              Criar aisense.toml
            </Button>
          </>
        ) : undefined
      }
    >
      {!lookup && !error && <p className="text-caption text-muted">Lendo o diretório…</p>}
      {lookup?.status === 'found' && <Commands path={lookup.path} config={lookup.config} />}
      {lookup?.status === 'invalid' && (
        <div className="flex flex-col gap-2">
          <p className="flex items-start gap-1.5 text-body text-primary">
            <AlertTriangle size={15} className="mt-0.5 shrink-0 text-awaiting" />O aisense.toml
            deste diretório não carregou. Os agentes seguem sem comandos até ele ser corrigido.
          </p>
          <p className="rounded-md bg-surface px-2.5 py-2 font-mono text-label text-failed">
            {lookup.problem.path}
            {lookup.problem.line !== null && `:${lookup.problem.line}`}
            {lookup.problem.column !== null && `:${lookup.problem.column}`} —{' '}
            {lookup.problem.message}
          </p>
        </div>
      )}
      {lookup?.status === 'missing' && lookup.proposal && (
        <div className="flex flex-col gap-2">
          <p className="text-body text-secondary">
            Não há aisense.toml. Encontramos{' '}
            <code className="font-mono">{lookup.proposal.detectedFrom}</code> e propomos o arquivo
            abaixo. Revise — os agentes vão rodar exatamente isto.
          </p>
          <textarea
            aria-label="Conteúdo proposto do aisense.toml"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            rows={16}
            spellCheck={false}
            className="rounded-md border border-strong bg-surface px-2.5 py-2 font-mono text-label text-primary focus:border-accent"
          />
        </div>
      )}
      {lookup?.status === 'missing' && !lookup.proposal && (
        <p className="flex items-start gap-1.5 text-body text-secondary">
          <FileCode size={15} className="mt-0.5 shrink-0" />
          Não há aisense.toml e não reconhecemos o tipo de projeto (pnpm, npm, Cargo, uv, Makefile).
          Crie o arquivo na raiz do diretório para os agentes saberem como operá-lo.
        </p>
      )}
      {error && (
        <p className="mt-2 text-caption text-failed" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}

function Commands({ path, config }: { path: string; config: ProjectConfig }) {
  const commands = Object.entries(config.commands);
  return (
    <div className="flex flex-col gap-3">
      <p className="truncate font-mono text-caption text-muted" title={path}>
        {path}
      </p>
      {commands.length === 0 ? (
        <p className="text-body text-secondary">O arquivo não define nenhum comando.</p>
      ) : (
        <table className="w-full text-left">
          <thead>
            <tr className="text-caption text-muted">
              <th className="py-1 font-normal">Comando</th>
              <th className="py-1 font-normal">Roda</th>
              <th className="py-1 text-right font-normal">Limite</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-subtle">
            {commands.map(([name, command]) =>
              command ? (
                <tr key={name}>
                  <td className="py-1.5 font-mono text-label text-primary">aisense run {name}</td>
                  <td className="py-1.5 font-mono text-label text-secondary">{command.run}</td>
                  <td className="py-1.5 text-right text-caption text-muted">
                    {Math.round(command.timeoutS / 60)} min
                  </td>
                </tr>
              ) : null,
            )}
          </tbody>
        </table>
      )}
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-caption">
        <dt className="text-muted">Bancada copia</dt>
        <dd className="font-mono text-secondary">
          {config.bench.copy.length ? config.bench.copy.join(', ') : '—'}
        </dd>
        <dt className="text-muted">Preparo da bancada</dt>
        <dd className="font-mono text-secondary">{config.bench.setup ?? '—'}</dd>
        <dt className="text-muted">Gate de revisão</dt>
        <dd className="font-mono text-secondary">
          {config.gates.review.length ? config.gates.review.join(', ') : '—'}
        </dd>
      </dl>
    </div>
  );
}
