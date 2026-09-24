import type { PlannedAgent } from '@/types/generated/PlannedAgent';
import type { RuntimeInfo } from '@/types/generated/RuntimeInfo';

/**
 * A primeira equipe tem de subir (F08-04): agente cujo runtime não está instalado vai
 * para o shell, que sempre existe. O modelo continua dizendo qual era o preferido, e a
 * tela avisa a troca.
 */
export function withRunnableRuntimes(
  plan: PlannedAgent[],
  runtimes: RuntimeInfo[],
): PlannedAgent[] {
  const available = (id: string) =>
    runtimes.some((r) => r.adapter.id === id && r.status.status === 'available');
  const fallback = available('shell') ? 'shell' : null;
  return plan.map((p) =>
    available(p.draft.adapterId) || !fallback
      ? p
      : { ...p, draft: { ...p.draft, adapterId: fallback } },
  );
}
