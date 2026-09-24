import { AlertTriangle } from 'lucide-react';
import { Button } from './Button';

/**
 * Erro com o que fazer a seguir (F08-03). `message` já traz a causa e, quando o core
 * mandou, a dica (`errorMessage`). Sem como tentar de novo, a saída é o diagnóstico.
 */
export function ErrorNotice({
  title,
  message,
  onRetry,
}: {
  title: string;
  message: string;
  onRetry?: () => void;
}) {
  return (
    <div
      role="alert"
      className="flex items-start gap-2 rounded-lg border border-failed/40 bg-surface p-3 text-left"
    >
      <AlertTriangle size={16} className="mt-0.5 shrink-0 text-failed" aria-hidden />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="text-body text-primary">{title}</p>
        <p className="text-caption break-words text-secondary">{message}</p>
        {!onRetry && (
          <p className="text-caption text-muted">
            Se continuar, exporte o diagnóstico em Configurações → Avançado e anexe ao relato.
          </p>
        )}
      </div>
      {onRetry && (
        <Button size="sm" onClick={onRetry}>
          Tentar de novo
        </Button>
      )}
    </div>
  );
}
