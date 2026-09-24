import { Pause, Play, RotateCw } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Button, Dialog, IconButton, Tooltip } from '@/components/ui';
import { errorMessage, onTeamProgress, teamsApi } from '@/features/teams/api';
import type { TeamOp } from '@/types/generated/TeamOp';
import type { TeamProgress } from '@/types/generated/TeamProgress';
import type { TeamStartReport } from '@/types/generated/TeamStartReport';
import { confirmationFor, describeProgress, progressRatio } from '../teamProgress';

interface TeamControlsProps {
  teamId: string;
  /** Quantos agentes da equipe estão rodando agora. */
  running: number;
  handleOf: (agentId: string) => string;
  onReport: (report: TeamStartReport) => void;
  onError: (message: string) => void;
}

/**
 * ▶ Iniciar equipe · ⏸ Parar tudo · ⟳ Reiniciar tudo (docs/09, T4; F03-06).
 * O core faz o trabalho de forma assíncrona e escalonada; aqui só se acompanha pelos
 * eventos `team:progress`, então a interface nunca espera um spawn.
 */
export function TeamControls({ teamId, running, handleOf, onReport, onError }: TeamControlsProps) {
  const [progress, setProgress] = useState<TeamProgress | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState<Exclude<TeamOp, 'start'> | null>(null);

  useEffect(() => {
    const unlisten = onTeamProgress((event) => {
      if (event.teamId === teamId) setProgress(event.finished ? null : event);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [teamId]);

  const run = async (op: TeamOp) => {
    setConfirming(null);
    setBusy(true);
    try {
      if (op === 'start') onReport(await teamsApi.start(teamId));
      else if (op === 'restart') onReport(await teamsApi.restart(teamId));
      else await teamsApi.stop(teamId);
    } catch (error: unknown) {
      onError(errorMessage(error));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  const ask = (op: Exclude<TeamOp, 'start'>) => {
    if (confirmationFor(op, running)) setConfirming(op);
    else void run(op);
  };

  return (
    <div className="flex items-center gap-1">
      {progress && (
        <div className="mr-2 flex w-44 flex-col gap-0.5" role="status" aria-live="polite">
          <span className="truncate text-caption text-secondary">
            {describeProgress(progress, handleOf)}
          </span>
          <span className="h-1 overflow-hidden rounded-full bg-hover">
            <span
              className="block h-full rounded-full bg-emphasis transition-[width] duration-200"
              style={{ width: `${progressRatio(progress) * 100}%` }}
            />
          </span>
        </div>
      )}
      <Button size="sm" onClick={() => void run('start')} disabled={busy}>
        <Play size={13} /> Iniciar equipe
      </Button>
      <Tooltip content="Parar tudo">
        <IconButton label="Parar tudo" onClick={() => ask('stop')} disabled={busy}>
          <Pause size={14} />
        </IconButton>
      </Tooltip>
      <Tooltip content="Reiniciar tudo">
        <IconButton label="Reiniciar tudo" onClick={() => ask('restart')} disabled={busy}>
          <RotateCw size={14} />
        </IconButton>
      </Tooltip>

      {confirming && (
        <Dialog
          open
          onOpenChange={(open) => !open && setConfirming(null)}
          title={confirming === 'stop' ? 'Parar tudo?' : 'Reiniciar tudo?'}
          description={confirmationFor(confirming, running) ?? ''}
          footer={
            <>
              <Button variant="ghost" onClick={() => setConfirming(null)}>
                Cancelar
              </Button>
              <Button
                variant={confirming === 'stop' ? 'danger' : 'primary'}
                onClick={() => void run(confirming)}
              >
                {confirming === 'stop' ? 'Parar tudo' : 'Reiniciar tudo'}
              </Button>
            </>
          }
        />
      )}
    </div>
  );
}
