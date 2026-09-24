import { ShieldAlert } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Button } from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import type { TeamId } from '@/types/generated/TeamId';
import { busApi, onBusBlocked } from './api';

/**
 * Aviso das guardas anti-laço (docs/07): o que foi barrado e, quando a equipe está
 * pausada pelo orçamento, o botão para continuar. Nenhum bloqueio é silencioso.
 */
export function GuardBanner({ teamId }: { teamId: TeamId }) {
  const [detail, setDetail] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  useEffect(() => {
    busApi
      .paused(teamId)
      .then(setPaused)
      .catch(() => {});
    const off = onBusBlocked((event) => {
      if (event.teamId !== teamId) return;
      setDetail(event.detail);
      if (event.reason.kind === 'teamBudget') setPaused(true);
    });
    return () => {
      void off.then((stop) => stop());
    };
  }, [teamId]);

  if (!detail && !paused) return null;

  const resume = () =>
    busApi
      .resume(teamId)
      .then(() => {
        setPaused(false);
        setDetail(null);
      })
      .catch((e: unknown) => setProblem(errorMessage(e)));

  return (
    <div
      role="alert"
      className="flex items-center gap-2 border-b border-subtle px-4 py-2 text-caption text-awaiting"
    >
      <ShieldAlert size={14} className="shrink-0" />
      <span className="min-w-0 flex-1">
        {detail ?? 'As entregas da equipe estão pausadas pelo limite de mensagens.'}
        {problem && <span className="text-failed"> {problem}</span>}
      </span>
      {paused ? (
        <Button size="sm" onClick={() => void resume()}>
          Continuar
        </Button>
      ) : (
        <Button size="sm" variant="ghost" onClick={() => setDetail(null)}>
          Dispensar
        </Button>
      )}
    </div>
  );
}
