import { AlertTriangle, Check, Minus, RefreshCw, X } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { IconButton, Tooltip } from '@/components/ui';
import { cn } from '@/lib/cn';
import type { RuntimeInfo } from '@/types/generated/RuntimeInfo';
import type { RuntimeOverview } from '@/types/generated/RuntimeOverview';
import { onAdaptersChanged, runtimesApi } from './api';

/**
 * Runtimes encontrados no sistema — o bloco "Encontramos no seu sistema" da T1
 * (docs/09). Indisponíveis aparecem com a dica de instalação do adaptador.
 */
export function RuntimeList() {
  const [overview, setOverview] = useState<RuntimeOverview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (refresh: boolean) => {
    setLoading(true);
    try {
      setOverview(await runtimesApi.overview(refresh));
      setError(null);
    } catch (e: unknown) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(false);
    const unlisten = onAdaptersChanged(() => void load(false));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [load]);

  return (
    <section aria-labelledby="runtimes-title" className="w-full max-w-md text-left">
      <header className="flex items-center justify-between pb-1.5">
        <h3 id="runtimes-title" className="text-caption tracking-[0.02em] text-muted uppercase">
          Encontramos no seu sistema
        </h3>
        <Tooltip content="Procurar de novo">
          <IconButton
            label="Procurar runtimes de novo"
            onClick={() => void load(true)}
            disabled={loading}
          >
            <RefreshCw size={14} className={cn(loading && 'animate-spin')} />
          </IconButton>
        </Tooltip>
      </header>

      {error && <p className="text-caption text-failed">Não foi possível verificar: {error}</p>}
      {!overview && !error && <p className="text-caption text-muted">Verificando…</p>}

      {overview && (
        <ul className="divide-y divide-subtle rounded-lg border border-subtle bg-surface">
          {overview.runtimes.map((runtime) => (
            <RuntimeRow key={runtime.adapter.id} runtime={runtime} />
          ))}
        </ul>
      )}

      {overview && overview.problems.length > 0 && (
        <ul className="mt-2 space-y-1">
          {overview.problems.map((problem) => (
            <li
              key={`${problem.path}:${problem.line ?? ''}`}
              className="flex gap-1.5 text-caption text-secondary"
            >
              <AlertTriangle size={13} className="mt-0.5 shrink-0 text-awaiting" />
              <span>
                <span className="font-mono">
                  {problem.path}
                  {problem.line !== null && `:${problem.line}`}
                </span>{' '}
                — {problem.message}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function RuntimeRow({ runtime }: { runtime: RuntimeInfo }) {
  const { adapter, status } = runtime;
  const { icon, label, color, detail } = describe(status);

  return (
    <li className="flex items-start gap-2.5 px-3 py-2">
      <span role="img" aria-label={label} className={cn('mt-0.5 shrink-0', color)}>
        {icon}
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-3">
          <span className="text-body text-primary">{adapter.name}</span>
          <span className="truncate text-caption text-muted" title={detail}>
            {detail}
          </span>
        </div>
        {status.status === 'missing' && (
          <p className="text-caption text-secondary">
            {adapter.installHint ? (
              <>
                Para instalar: <code className="font-mono">{adapter.installHint}</code>
              </>
            ) : (
              status.reason
            )}
          </p>
        )}
      </div>
    </li>
  );
}

function describe(status: RuntimeInfo['status']) {
  switch (status.status) {
    case 'available':
      return {
        icon: <Check size={14} />,
        label: 'Disponível',
        color: 'text-idle',
        detail: status.version ?? status.path,
      };
    case 'missing':
      return {
        icon: <X size={14} />,
        label: 'Indisponível',
        color: 'text-failed',
        detail: 'não encontrado',
      };
    case 'perAgent':
      return {
        icon: <Minus size={14} />,
        label: 'Definido em cada agente',
        color: 'text-muted',
        detail: 'definido em cada agente',
      };
  }
}

function errorMessage(e: unknown): string {
  if (typeof e === 'object' && e !== null && 'message' in e) return String(e.message);
  return String(e);
}
