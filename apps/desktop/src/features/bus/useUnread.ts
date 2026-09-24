import { useCallback, useEffect, useState } from 'react';
import type { PendingBadge } from '@/features/team-room/components/AgentSidebar';
import type { Agent } from '@/types/generated/Agent';
import type { TeamId } from '@/types/generated/TeamId';
import { busApi, onBusMessage, onBusRead } from './api';

/**
 * Não lidas por agente, ao vivo: relê a contagem a cada mensagem da equipe e a cada
 * leitura (`bus:message`, `bus:read`). A cor do badge é a de quem mandou por último.
 */
export function useUnread(teamId: TeamId, agents: Agent[]) {
  const [counts, setCounts] = useState<Record<string, number>>({});
  const [lastFrom, setLastFrom] = useState<Record<string, string>>({});

  const reload = useCallback(() => {
    busApi
      .unread(teamId)
      .then((list) => setCounts(Object.fromEntries(list.map((u) => [u.agentId, u.count]))))
      .catch(() => {});
  }, [teamId]);

  useEffect(() => {
    reload();
    const offMessage = onBusMessage((event) => {
      if (event.teamId !== teamId) return;
      if (event.message.to.startsWith('@') && event.message.to !== '@all') {
        const to = event.message.to.slice(1);
        setLastFrom((prev) => ({ ...prev, [to]: event.message.from }));
      }
      reload();
    });
    const offRead = onBusRead(() => reload());
    return () => {
      void offMessage.then((stop) => stop());
      void offRead.then((stop) => stop());
    };
  }, [teamId, reload]);

  const pendingOf = useCallback(
    (id: string): PendingBadge | undefined => {
      const count = counts[id] ?? 0;
      if (count === 0) return undefined;
      const agent = agents.find((a) => a.id === id);
      const senderHandle = agent ? lastFrom[agent.handle]?.replace(/^@/, '') : undefined;
      const sender = agents.find((a) => a.handle === senderHandle);
      const color = sender?.color ?? agent?.color;
      return color ? { count, color } : undefined;
    },
    [counts, lastFrom, agents],
  );

  return { counts, pendingOf, reload };
}
