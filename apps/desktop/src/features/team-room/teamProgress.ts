import type { TeamOp } from '@/types/generated/TeamOp';
import type { TeamProgress } from '@/types/generated/TeamProgress';

const VERB: Record<TeamOp, string> = {
  start: 'Iniciando',
  stop: 'Parando',
  restart: 'Reiniciando',
};

/** "Iniciando 3/6 · @backend" — o texto da barra de progresso da equipe. */
export function describeProgress(
  progress: TeamProgress,
  handleOf: (agentId: string) => string,
): string {
  const count = `${Math.min(progress.done + 1, progress.total)}/${progress.total}`;
  const who = progress.agentId ? ` · @${handleOf(progress.agentId)}` : '';
  return `${VERB[progress.op]} ${count}${who}`;
}

/** Fração concluída, de 0 a 1 (equipe sem alvos conta como concluída). */
export function progressRatio(progress: TeamProgress): number {
  if (progress.total === 0) return 1;
  return Math.min(1, progress.done / progress.total);
}

/**
 * Texto da confirmação de ⏸/⟳, ou `null` quando não há o que confirmar (ninguém
 * rodando: parar é inofensivo e reiniciar vira iniciar).
 */
export function confirmationFor(op: Exclude<TeamOp, 'start'>, running: number): string | null {
  if (running === 0) return null;
  const agents = running === 1 ? '1 agente' : `${running} agentes`;
  return op === 'stop'
    ? `Parar ${agents} em execução? O que eles estiverem fazendo é interrompido.`
    : `Reiniciar ${agents} em execução? Cada um volta do zero, um por vez.`;
}
