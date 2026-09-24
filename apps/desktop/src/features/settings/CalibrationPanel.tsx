import { Gauge, RefreshCw } from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useState } from 'react';
import { Button, IconButton, Tooltip } from '@/components/ui';
import { agentsApi } from '@/features/agents/api';
import { runtimesApi } from '@/features/runtimes/api';
import { errorMessage } from '@/features/teams/api';
import { useTeams } from '@/features/teams/store';
import { cn } from '@/lib/cn';
import type { Adapter } from '@/types/generated/Adapter';
import type { Agent } from '@/types/generated/Agent';
import type { AgentState } from '@/types/generated/AgentState';
import type { Calibration } from '@/types/generated/Calibration';
import type { StateRules } from '@/types/generated/StateRules';
import { settingsApi } from './api';
import { STATE_LABEL, stateOfField } from './calibration';

/** Enquanto um agente é a fonte, a tela é relida neste ritmo. */
const SCREEN_REFRESH_MS = 1000;

/**
 * Modo calibração do detector de estado (T9 → Runtimes; `docs/05`, "Calibração").
 * Mostra a tela de um agente rodando como os regex a enxergam, testa as regras a cada
 * tecla e, ao aplicar, grava no adaptador e vale na hora para as sessões vivas.
 */
export function CalibrationPanel() {
  const [adapters, setAdapters] = useState<Adapter[]>([]);
  const [adapterId, setAdapterId] = useState('');
  const [rules, setRules] = useState<StateRules | null>(null);
  const [agents, setAgents] = useState<{ agent: Agent; state: AgentState }[]>([]);
  const [agentId, setAgentId] = useState('');
  const [screen, setScreen] = useState('');
  const [result, setResult] = useState<Calibration | null>(null);
  const [status, setStatus] = useState<{ tone: 'ok' | 'error'; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const teams = useTeams((s) => s.teams);
  const ids = { adapter: useId(), agent: useId(), screen: useId() };

  const loadAdapters = useCallback(async () => {
    try {
      const overview = await runtimesApi.overview(false);
      setAdapters(overview.runtimes.map((r) => r.adapter));
    } catch (e: unknown) {
      setStatus({ tone: 'error', text: errorMessage(e) });
    }
  }, []);
  useEffect(() => void loadAdapters(), [loadAdapters]);

  const adapter = adapters.find((a) => a.id === adapterId) ?? null;
  // Trocar de runtime recomeça das regras dele.
  useEffect(() => {
    setRules(adapter ? { ...adapter.state } : null);
    setAgentId('');
    setStatus(null);
  }, [adapter]);

  // Agentes rodando com este runtime, em todas as equipes.
  useEffect(() => {
    if (!adapterId) {
      setAgents([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      const found: { agent: Agent; state: AgentState }[] = [];
      for (const summary of teams ?? []) {
        const list = await agentsApi.list(summary.team.id).catch(() => [] as Agent[]);
        for (const agent of list.filter((a) => a.adapterId === adapterId)) {
          const state = await agentsApi.state(agent.id).catch(() => 'stopped' as AgentState);
          if (state !== 'stopped') found.push({ agent, state });
        }
      }
      if (!cancelled) setAgents(found);
    })();
    return () => {
      cancelled = true;
    };
  }, [adapterId, teams]);

  // Tela ao vivo do agente escolhido.
  useEffect(() => {
    if (!agentId) return;
    let alive = true;
    const read = () =>
      void settingsApi
        .calibrationScreen(agentId)
        .then((text) => alive && setScreen(text))
        .catch(() => {});
    read();
    const timer = window.setInterval(read, SCREEN_REFRESH_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [agentId]);

  // Teste a cada mudança de regra ou tela.
  useEffect(() => {
    if (!rules) {
      setResult(null);
      return;
    }
    let alive = true;
    void settingsApi
      .calibrationTest(rules, screen)
      .then((r) => alive && setResult(r))
      .catch((e: unknown) => alive && setStatus({ tone: 'error', text: errorMessage(e) }));
    return () => {
      alive = false;
    };
  }, [rules, screen]);

  const changed = useMemo(
    () =>
      adapter !== null && rules !== null && JSON.stringify(adapter.state) !== JSON.stringify(rules),
    [adapter, rules],
  );

  const apply = async () => {
    if (!adapter || !rules) return;
    setBusy(true);
    try {
      const path = await settingsApi.calibrationApply(adapter.id, rules);
      setStatus({
        tone: 'ok',
        text: `Gravado em ${path}. Os agentes de ${adapter.name} já usam as regras novas.`,
      });
      await loadAdapters();
    } catch (e: unknown) {
      setStatus({ tone: 'error', text: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  };

  const setField = (field: keyof StateRules, value: string) =>
    setRules((r) =>
      r
        ? field === 'quietMs'
          ? { ...r, quietMs: Math.max(0, Number(value) || 0) }
          : { ...r, [field]: value === '' ? null : value }
        : r,
    );

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-end gap-3">
        <div className="flex flex-col gap-1">
          <label htmlFor={ids.adapter} className="text-label text-secondary">
            Runtime
          </label>
          <select
            id={ids.adapter}
            value={adapterId}
            onChange={(e) => setAdapterId(e.target.value)}
            className="h-8 rounded-md border border-strong bg-surface px-2 text-body text-primary"
          >
            <option value="">Escolha…</option>
            {adapters.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
                {a.source.kind === 'user' ? ' (seu arquivo)' : ''}
              </option>
            ))}
          </select>
        </div>
        {adapter && (
          <div className="flex flex-col gap-1">
            <label htmlFor={ids.agent} className="text-label text-secondary">
              Tela de um agente rodando
            </label>
            <select
              id={ids.agent}
              value={agentId}
              onChange={(e) => setAgentId(e.target.value)}
              className="h-8 rounded-md border border-strong bg-surface px-2 text-body text-primary"
            >
              <option value="">
                {agents.length ? 'Escolha…' : 'Nenhum agente deste runtime rodando'}
              </option>
              {agents.map(({ agent, state }) => (
                <option key={agent.id} value={agent.id}>
                  @{agent.handle} — {STATE_LABEL[state] ?? state}
                </option>
              ))}
            </select>
          </div>
        )}
      </div>

      {!adapter && (
        <p className="flex items-center gap-2 text-caption text-muted">
          <Gauge size={14} aria-hidden />
          Escolha um runtime para ajustar como o aisense reconhece ocioso, ocupado e aguardando.
        </p>
      )}

      {adapter && rules && (
        <>
          <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
            <RegexField
              label="Aguardando (awaiting_regex)"
              value={rules.awaitingRegex ?? ''}
              error={result?.patterns.find((p) => p.field === 'awaiting_regex')?.error}
              onChange={(v) => setField('awaitingRegex', v)}
            />
            <RegexField
              label="Ocupado (busy_regex)"
              value={rules.busyRegex ?? ''}
              error={result?.patterns.find((p) => p.field === 'busy_regex')?.error}
              onChange={(v) => setField('busyRegex', v)}
            />
            <RegexField
              label="Ocioso (idle_regex)"
              value={rules.idleRegex ?? ''}
              error={result?.patterns.find((p) => p.field === 'idle_regex')?.error}
              onChange={(v) => setField('idleRegex', v)}
            />
            <div className="flex flex-col gap-1">
              <label className="text-label text-secondary" htmlFor={`${ids.screen}-quiet`}>
                Silêncio mínimo (quiet_ms)
              </label>
              <input
                id={`${ids.screen}-quiet`}
                inputMode="numeric"
                value={rules.quietMs}
                onChange={(e) => setField('quietMs', e.target.value)}
                className="h-8 w-28 rounded-md border border-strong bg-surface px-2.5 font-mono text-label text-primary focus:border-emphasis"
              />
              <span className="text-caption text-muted">Entre 50 e 10000 ms.</span>
            </div>
          </div>

          <div className="flex flex-col gap-1">
            <div className="flex items-center justify-between">
              <label htmlFor={ids.screen} className="text-label text-secondary">
                Tela {agentId ? '(ao vivo)' : '(cole um trecho para testar)'}
              </label>
              {agentId && (
                <Tooltip content="Relida a cada segundo">
                  <span className="flex items-center gap-1 text-caption text-muted">
                    <RefreshCw size={12} aria-hidden /> ao vivo
                  </span>
                </Tooltip>
              )}
            </div>
            <textarea
              id={ids.screen}
              value={screen}
              readOnly={Boolean(agentId)}
              onChange={(e) => setScreen(e.target.value)}
              rows={4}
              placeholder="$ "
              className="rounded-md border border-strong bg-terminal p-2 font-mono text-label text-primary"
            />
          </div>

          {result && <Verdict result={result} />}

          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="primary"
              onClick={() => void apply()}
              disabled={!changed || busy || (result?.patterns.some((p) => p.error) ?? false)}
            >
              Aplicar sem reiniciar
            </Button>
            <Button
              variant="ghost"
              onClick={() => setRules({ ...adapter.state })}
              disabled={!changed || busy}
            >
              Desfazer
            </Button>
            <IconButton label="Recarregar runtimes" onClick={() => void loadAdapters()}>
              <RefreshCw size={14} />
            </IconButton>
          </div>
          <p className="text-caption text-muted">
            {adapter.source.kind === 'user'
              ? `Grava em ${adapter.source.path}. Comentários do arquivo não são mantidos.`
              : 'O adaptador embutido não muda: uma cópia com as regras novas vai para a sua pasta de adaptadores e passa a valer no lugar dele.'}
          </p>
        </>
      )}

      {status && (
        <p
          role={status.tone === 'error' ? 'alert' : 'status'}
          className={cn('text-caption', status.tone === 'error' ? 'text-failed' : 'text-idle')}
        >
          {status.text}
        </p>
      )}
    </div>
  );
}

function RegexField({
  label,
  value,
  error,
  onChange,
}: {
  label: string;
  value: string;
  error?: string | null;
  onChange: (value: string) => void;
}) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="text-label text-secondary">
        {label}
      </label>
      <input
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        spellCheck={false}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? `${id}-error` : undefined}
        className={cn(
          'h-8 rounded-md border bg-surface px-2.5 font-mono text-label text-primary',
          error ? 'border-failed' : 'border-strong focus:border-emphasis',
        )}
      />
      {error && (
        <p id={`${id}-error`} className="font-mono text-caption whitespace-pre-wrap text-failed">
          {error}
        </p>
      )}
    </div>
  );
}

/** As linhas que os regex veem, com a que decidiu marcada. */
function Verdict({ result }: { result: Calibration }) {
  const decided = result.decided;
  return (
    <div className="flex flex-col gap-1.5" aria-live="polite">
      <p className="text-body text-primary">
        {decided ? (
          <>
            O detector diria <strong>{STATE_LABEL[decided]}</strong>
            {result.decidedLine !== null && ` (linha ${result.decidedLine + 1})`}.
          </>
        ) : (
          'Nada casou: o agente fica como está e, depois de 60 s de silêncio, vira ocioso com confiança baixa.'
        )}
      </p>
      {result.lines.length > 0 && (
        <ol className="rounded-md border border-subtle bg-surface py-1 font-mono text-caption">
          {result.lines.map((line, i) => {
            const field = result.patterns.find((p) => p.matches.includes(i))?.field;
            const state = field ? stateOfField(field) : null;
            return (
              <li
                // biome-ignore lint/suspicious/noArrayIndexKey: a posição da linha é a identidade dela
                key={i}
                className={cn(
                  'flex gap-2 px-2 whitespace-pre',
                  i === result.decidedLine && 'bg-hover',
                )}
              >
                <span className="w-5 shrink-0 text-right text-muted">{i + 1}</span>
                <span className="min-w-0 flex-1 truncate text-primary">{line}</span>
                {state && <span className="shrink-0 text-secondary">{STATE_LABEL[state]}</span>}
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
}
