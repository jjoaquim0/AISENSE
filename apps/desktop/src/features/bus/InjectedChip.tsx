import { MessageSquareText } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Badge } from '@/components/ui';
import { onBusInjected } from './api';

/** Quanto tempo o chip fica depois de uma injeção. */
export const INJECTED_CHIP_MS = 6_000;

/**
 * "mensagem injetada" (docs/07, modo `push`): o AISENSE digitou no terminal deste agente.
 * Nada é injetado sem aparecer (ADR 0006).
 */
export function InjectedChip({ agentId }: { agentId: string }) {
  const [count, setCount] = useState(0);
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const off = onBusInjected((event) => {
      if (event.agentId !== agentId) return;
      setCount(event.messageIds.length);
      clearTimeout(timer);
      timer = setTimeout(() => setCount(0), INJECTED_CHIP_MS);
    });
    return () => {
      clearTimeout(timer);
      void off.then((stop) => stop());
    };
  }, [agentId]);
  if (count === 0) return null;
  return (
    <Badge variant="outline" className="text-awaiting">
      <MessageSquareText size={11} />
      {count === 1 ? 'mensagem injetada' : `${count} mensagens injetadas`}
    </Badge>
  );
}
