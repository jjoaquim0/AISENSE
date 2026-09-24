import { KeyRound, Settings as SettingsIcon, Trash2 } from 'lucide-react';
import { type ReactNode, useEffect, useId, useState } from 'react';
import { Button, EmptyState, IconButton, Input, Kbd } from '@/components/ui';
import { formatShortcut } from '@/components/ui/Kbd';
import { runtimesApi } from '@/features/runtimes/api';
import { RuntimeList } from '@/features/runtimes/RuntimeList';
import { useNav } from '@/features/shell/nav';
import { errorMessage } from '@/features/teams/api';
import { cn } from '@/lib/cn';
import { recordCombo, SHORTCUT_CATALOG } from '@/lib/shortcuts';
import type { Adapter } from '@/types/generated/Adapter';
import type { AppSettings } from '@/types/generated/AppSettings';
import type { SettingsView } from '@/types/generated/SettingsView';
import { settingsApi } from './api';
import { CalibrationPanel } from './CalibrationPanel';
import { Group, NumberField, PathLine, Radios, Toggle } from './fields';
import { useSettings } from './store';

export const SECTIONS = [
  ['appearance', 'Aparência'],
  ['runtimes', 'Runtimes'],
  ['skills', 'Skills'],
  ['bus', 'Barramento'],
  ['notifications', 'Notificações'],
  ['session', 'Sessão'],
  ['shortcuts', 'Atalhos'],
  ['secrets', 'Segredos'],
  ['advanced', 'Avançado'],
] as const;

export type SectionId = (typeof SECTIONS)[number][0];

type Update = (change: (draft: AppSettings) => void) => void;

/** T9 — Configurações (docs/09; F08-05). Cada mudança grava na hora. */
export function SettingsScreen() {
  const view = useSettings((s) => s.view);
  const error = useSettings((s) => s.error);
  const load = useSettings((s) => s.load);
  const save = useSettings((s) => s.update);
  const [section, setSection] = useState<SectionId>('appearance');
  const update: Update = (change) => void save(change);

  useEffect(() => {
    if (!view) void load();
  }, [view, load]);

  if (!view) {
    return error ? (
      <EmptyState
        icon={<SettingsIcon size={22} />}
        title="Não foi possível abrir as configurações"
        description={error}
        action={<Button onClick={() => void load()}>Tentar de novo</Button>}
      />
    ) : (
      <SettingsSkeleton />
    );
  }

  return (
    <div className="flex h-full min-h-0">
      <nav
        aria-label="Seções das configurações"
        className="flex w-48 shrink-0 flex-col gap-0.5 border-r border-subtle p-2"
      >
        <h2 className="px-2 pb-2 text-heading text-primary">Configurações</h2>
        {SECTIONS.map(([id, label]) => (
          <button
            key={id}
            type="button"
            aria-current={section === id ? 'page' : undefined}
            onClick={() => setSection(id)}
            className={cn(
              'rounded-md px-2 py-1.5 text-left text-body transition-colors duration-100',
              section === id
                ? 'bg-hover text-primary'
                : 'text-secondary hover:bg-hover hover:text-primary',
            )}
          >
            {label}
          </button>
        ))}
      </nav>
      <div className="min-w-0 flex-1 overflow-auto">
        <div className="mx-auto flex max-w-2xl flex-col gap-5 p-6">
          {view.warning && (
            <p
              role="alert"
              className="rounded-md border border-awaiting p-2 text-caption text-primary"
            >
              {view.warning}
            </p>
          )}
          {error && (
            <p role="alert" className="text-caption text-failed">
              Não foi possível gravar: {error}
            </p>
          )}
          <Section id={section} view={view} update={update} />
        </div>
      </div>
    </div>
  );
}

function Section({ id, view, update }: { id: SectionId; view: SettingsView; update: Update }) {
  const s = view.settings;
  switch (id) {
    case 'appearance':
      return <AppearanceSection settings={s} update={update} />;
    case 'runtimes':
      return (
        <>
          <Group
            title="Runtimes detectados"
            description="Os adaptadores moram em TOML; edite-os na pasta abaixo."
          >
            <RuntimeList />
            <PathLine label="Pasta de adaptadores" path={view.adaptersDir} />
          </Group>
          <Group
            title="Modo calibração"
            description="Ajuste os regex do detector de estado vendo a tela real de um agente. Aplicar vale sem reiniciar."
          >
            <CalibrationPanel />
          </Group>
        </>
      );
    case 'skills':
      return <SkillsSection dataDir={view.dataDir} />;
    case 'bus':
      return <BusSection settings={s} update={update} />;
    case 'notifications':
      return <NotificationsSection settings={s} update={update} />;
    case 'session':
      return (
        <Group title="Ao abrir o app">
          <Toggle
            label="Reabrir a última equipe"
            hint="Com a vista, o layout dos painéis e os tamanhos como você deixou."
            checked={s.session.restoreLastTeam}
            onChange={(v) =>
              update((d) => {
                d.session.restoreLastTeam = v;
              })
            }
          />
          <Toggle
            label="Religar os agentes"
            hint="Quem estava rodando quando o app fechou sobe de novo, em qualquer equipe."
            checked={s.session.relaunchAgents}
            onChange={(v) =>
              update((d) => {
                d.session.relaunchAgents = v;
              })
            }
          />
        </Group>
      );
    case 'shortcuts':
      return <ShortcutsSection settings={s} update={update} />;
    case 'secrets':
      return <SecretsSection settings={s} />;
    case 'advanced':
      return <AdvancedSection view={view} update={update} />;
  }
}

function AppearanceSection({ settings, update }: { settings: AppSettings; update: Update }) {
  const a = settings.appearance;
  return (
    <Group title="Aparência">
      <Radios
        legend="Tema"
        value={a.theme}
        options={[
          ['light', 'Claro'],
          ['dark', 'Escuro'],
          ['system', 'Sistema'],
        ]}
        onChange={(v) =>
          update((d) => {
            d.appearance.theme = v;
          })
        }
      />
      <Radios
        legend="Densidade do terminal"
        value={a.density}
        options={[
          ['comfortable', 'Confortável'],
          ['compact', 'Compacta'],
        ]}
        onChange={(v) =>
          update((d) => {
            d.appearance.density = v;
          })
        }
      />
      <NumberField
        label="Tamanho da fonte do terminal"
        value={a.terminalFontSize}
        min={10}
        max={24}
        suffix="px"
        hint="Vale na hora para os terminais abertos."
        onCommit={(v) =>
          update((d) => {
            d.appearance.terminalFontSize = v;
          })
        }
      />
      <CommitInput
        label="Fonte do terminal"
        value={a.terminalFontFamily}
        placeholder="JetBrains Mono (padrão)"
        hint="Uma fonte monoespaçada instalada no sistema. Vazio usa a do AISENSE."
        onCommit={(v) =>
          update((d) => {
            d.appearance.terminalFontFamily = v;
          })
        }
      />
    </Group>
  );
}

function SkillsSection({ dataDir }: { dataDir: string }) {
  const go = useNav((s) => s.go);
  return (
    <Group
      title="Biblioteca de skills"
      description="As suas skills ficam numa pasta por skill, com o SKILL.md dentro. Importar e exportar ficam na biblioteca."
    >
      <PathLine label="Pasta da biblioteca" path={`${dataDir}/skills`} />
      <div>
        <Button onClick={() => go('skills')}>Abrir a biblioteca</Button>
      </div>
    </Group>
  );
}

function BusSection({ settings, update }: { settings: AppSettings; update: Update }) {
  const b = settings.bus;
  return (
    <>
      <Group
        title="Limites anti-laço"
        description="Valem só para mensagens de agentes; as suas passam sempre. Mudanças valem na hora."
      >
        <div className="grid grid-cols-2 gap-3">
          <NumberField
            label="Mensagens por agente"
            suffix="por minuto"
            value={b.guards.perAgentPerMinute}
            min={1}
            max={10_000}
            onCommit={(v) =>
              update((d) => {
                d.bus.guards.perAgentPerMinute = v;
              })
            }
          />
          <NumberField
            label="Mensagens da equipe"
            suffix="por hora"
            hint="Ao estourar, a equipe pausa até você liberar."
            value={b.guards.teamPerHour}
            min={1}
            max={100_000}
            onCommit={(v) =>
              update((d) => {
                d.bus.guards.teamPerHour = v;
              })
            }
          />
          <NumberField
            label="Repetições idênticas"
            hint="A seguinte é bloqueada."
            value={b.guards.maxIdentical}
            min={1}
            max={100}
            onCommit={(v) =>
              update((d) => {
                d.bus.guards.maxIdentical = v;
              })
            }
          />
          <NumberField
            label="Profundidade de respostas"
            hint="Acima disso, o remetente recebe um aviso."
            value={b.guards.maxReplyDepth}
            min={1}
            max={1_000}
            onCommit={(v) =>
              update((d) => {
                d.bus.guards.maxReplyDepth = v;
              })
            }
          />
        </div>
      </Group>
      <Group title="Tempo e histórico">
        <NumberField
          label="Timeout padrão do aisense ask"
          suffix="segundos"
          hint="Quando o agente não informa --timeout. Máximo de 30 minutos."
          value={b.askDefaultSecs}
          min={10}
          max={1_800}
          onCommit={(v) =>
            update((d) => {
              d.bus.askDefaultSecs = v;
            })
          }
        />
        <NumberField
          label="Guardar mensagens por"
          suffix="dias"
          hint="Mensagens mais antigas são apagadas na subida e a cada 6 horas."
          value={b.retentionDays}
          min={1}
          max={3_650}
          onCommit={(v) =>
            update((d) => {
              d.bus.retentionDays = v;
            })
          }
        />
      </Group>
    </>
  );
}

const HOUR = 60 * 60 * 1000;

function NotificationsSection({ settings, update }: { settings: AppSettings; update: Update }) {
  const n = settings.notifications;
  const mutedUntil = n.mutedUntil && n.mutedUntil > Date.now() ? n.mutedUntil : null;
  return (
    <Group
      title="Notificações do sistema"
      description="Só com o app em segundo plano, ou com outra equipe aberta: nunca para a equipe que você está olhando."
    >
      <Toggle
        label="Notificar"
        checked={n.enabled}
        onChange={(v) =>
          update((d) => {
            d.notifications.enabled = v;
          })
        }
      />
      <div className="flex flex-col gap-2 pl-5">
        <Toggle
          label="Quando um agente espera por você"
          checked={n.awaitingInput}
          disabled={!n.enabled}
          onChange={(v) =>
            update((d) => {
              d.notifications.awaitingInput = v;
            })
          }
        />
        <Toggle
          label="Quando um agente falha"
          checked={n.failed}
          disabled={!n.enabled}
          onChange={(v) =>
            update((d) => {
              d.notifications.failed = v;
            })
          }
        />
      </div>
      <div className="flex flex-wrap items-center gap-2">
        {mutedUntil ? (
          <>
            <span className="text-caption text-secondary">
              Silenciado até{' '}
              {new Date(mutedUntil).toLocaleTimeString('pt-BR', {
                hour: '2-digit',
                minute: '2-digit',
              })}
              .
            </span>
            <Button
              size="sm"
              onClick={() =>
                update((d) => {
                  d.notifications.mutedUntil = null;
                })
              }
            >
              Reativar agora
            </Button>
          </>
        ) : (
          <>
            <Button
              size="sm"
              disabled={!n.enabled}
              onClick={() =>
                update((d) => {
                  d.notifications.mutedUntil = Date.now() + HOUR;
                })
              }
            >
              Silenciar por 1 hora
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={!n.enabled}
              onClick={() =>
                update((d) => {
                  d.notifications.mutedUntil = Date.now() + 8 * HOUR;
                })
              }
            >
              Silenciar por 8 horas
            </Button>
          </>
        )}
      </div>
    </Group>
  );
}

function ShortcutsSection({ settings, update }: { settings: AppSettings; update: Update }) {
  const [recording, setRecording] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const overrides = settings.shortcuts as Record<string, string>;
  const effective = (combo: string) => overrides[combo] ?? combo;

  useEffect(() => {
    if (!recording) return;
    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === 'Escape') {
        setRecording(null);
        return;
      }
      const combo = recordCombo(event);
      if (!combo) return;
      const taken = SHORTCUT_CATALOG.find(
        (s) => s.combo !== recording && effective(s.combo) === combo,
      );
      if (taken) {
        setProblem(`${formatShortcut(combo)} já é “${taken.label}”.`);
        return;
      }
      setProblem(null);
      setRecording(null);
      update((d) => {
        const map = d.shortcuts as Record<string, string>;
        if (combo === recording) delete map[recording];
        else map[recording] = combo;
      });
    };
    // Captura: antes da camada de atalhos, que executaria a ação da tecla.
    window.addEventListener('keydown', onKey, { capture: true });
    return () => window.removeEventListener('keydown', onKey, { capture: true });
  });

  const groups = [...new Set(SHORTCUT_CATALOG.map((s) => s.group))];
  return (
    <Group
      title="Atalhos"
      description="Clique em um atalho e pressione a combinação nova (com ⌘/Ctrl). Esc cancela. ⌘1..9 e Esc Esc são fixos."
    >
      {groups.map((group) => (
        <div key={group} className="flex flex-col gap-1">
          <h4 className="text-caption tracking-[0.02em] text-muted uppercase">{group}</h4>
          <ul className="divide-y divide-subtle rounded-lg border border-subtle">
            {SHORTCUT_CATALOG.filter((s) => s.group === group).map((s) => (
              <li key={s.combo} className="flex items-center justify-between gap-2 px-3 py-1.5">
                <span className="text-body text-primary">{s.label}</span>
                <span className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => {
                      setProblem(null);
                      setRecording(s.combo);
                    }}
                    aria-label={`Mudar o atalho de ${s.label}`}
                    className="rounded-md px-1.5 py-0.5 hover:bg-hover"
                  >
                    {recording === s.combo ? (
                      <span className="text-caption text-accent">Pressione…</span>
                    ) : (
                      <Kbd>{formatShortcut(effective(s.combo))}</Kbd>
                    )}
                  </button>
                  {overrides[s.combo] && (
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() =>
                        update((d) => void delete (d.shortcuts as Record<string, string>)[s.combo])
                      }
                    >
                      Padrão
                    </Button>
                  )}
                </span>
              </li>
            ))}
          </ul>
        </div>
      ))}
      {problem && (
        <p role="alert" className="text-caption text-failed">
          {problem}
        </p>
      )}
    </Group>
  );
}

function SecretsSection({ settings }: { settings: AppSettings }) {
  const received = useSettings((s) => s.received);
  const [adapters, setAdapters] = useState<Adapter[]>([]);
  const [adapterId, setAdapterId] = useState('');
  const [envName, setEnvName] = useState('');
  const [value, setValue] = useState('');
  const [problem, setProblem] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const selectId = useId();

  useEffect(() => {
    void runtimesApi
      .overview(false)
      .then((o) => setAdapters(o.runtimes.map((r) => r.adapter)))
      .catch((e: unknown) => setProblem(errorMessage(e)));
  }, []);

  const add = async () => {
    setSaving(true);
    try {
      received(await settingsApi.setSecret(adapterId, envName, value));
      setEnvName('');
      setValue('');
      setProblem(null);
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (adapter: string, env: string) => {
    try {
      received(await settingsApi.deleteSecret(adapter, env));
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  const nameOf = (id: string) => adapters.find((a) => a.id === id)?.name ?? id;
  return (
    <Group
      title="Segredos"
      description="Chaves de API guardadas no keychain do sistema, nunca em arquivo. Cada agente do runtime recebe a variável no próximo início."
    >
      {settings.secrets.length === 0 ? (
        <p className="flex items-center gap-2 text-caption text-muted">
          <KeyRound size={14} aria-hidden /> Nenhum segredo guardado.
        </p>
      ) : (
        <ul className="divide-y divide-subtle rounded-lg border border-subtle">
          {settings.secrets.map((s) => (
            <li key={`${s.adapterId}/${s.envName}`} className="flex items-center gap-2 px-3 py-1.5">
              <code className="font-mono text-label text-primary">{s.envName}</code>
              <span className="flex-1 text-caption text-muted">para {nameOf(s.adapterId)}</span>
              <span className="font-mono text-caption text-muted">
                <span aria-hidden>{s.masked || '••••••••'}</span>
                <span className="sr-only">valor oculto</span>
              </span>
              <IconButton
                label={`Apagar ${s.envName} de ${nameOf(s.adapterId)}`}
                onClick={() => void remove(s.adapterId, s.envName)}
              >
                <Trash2 size={14} />
              </IconButton>
            </li>
          ))}
        </ul>
      )}
      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <div className="flex flex-col gap-1">
          <label htmlFor={selectId} className="text-label text-secondary">
            Runtime
          </label>
          <select
            id={selectId}
            value={adapterId}
            onChange={(e) => setAdapterId(e.target.value)}
            className="h-8 rounded-md border border-strong bg-surface px-2 text-body text-primary"
          >
            <option value="">Escolha…</option>
            {adapters.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </div>
        <Input
          label="Variável"
          value={envName}
          placeholder="ANTHROPIC_API_KEY"
          onChange={(e) => setEnvName(e.target.value.toUpperCase())}
          className="font-mono"
        />
        <Input
          label="Valor"
          type="password"
          autoComplete="off"
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <Button
          type="submit"
          variant="primary"
          disabled={!adapterId || !envName || !value || saving}
        >
          Guardar
        </Button>
      </form>
      {problem && (
        <p role="alert" className="text-caption text-failed">
          {problem}
        </p>
      )}
    </Group>
  );
}

function AdvancedSection({ view, update }: { view: SettingsView; update: Update }) {
  const [message, setMessage] = useState<{ tone: 'ok' | 'error'; text: string } | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const received = useSettings((s) => s.received);
  const logId = useId();

  const exportDiagnostics = async () => {
    try {
      const path = await settingsApi.exportDiagnostics();
      setMessage({
        tone: 'ok',
        text: `Diagnóstico gravado em ${path}. Anexe-o ao relato do problema.`,
      });
    } catch (e: unknown) {
      setMessage({ tone: 'error', text: errorMessage(e) });
    }
  };

  const reset = async () => {
    try {
      received(await settingsApi.reset());
      setConfirmReset(false);
      setMessage({
        tone: 'ok',
        text: 'Preferências de volta ao padrão. Os segredos continuam guardados.',
      });
    } catch (e: unknown) {
      setMessage({ tone: 'error', text: errorMessage(e) });
    }
  };

  return (
    <>
      <Group title="Dados">
        <PathLine label="Diretório de dados" path={view.dataDir} />
        <PathLine label="Arquivo de preferências" path={view.settingsPath} />
        <p className="text-caption text-muted">
          Para mudar o diretório, defina a variável de ambiente AISENSE_HOME antes de abrir o app.
        </p>
      </Group>
      <Group title="Diagnóstico">
        <div className="flex flex-col gap-1">
          <label htmlFor={logId} className="text-label text-secondary">
            Nível de log
          </label>
          <select
            id={logId}
            value={view.settings.advanced.logLevel}
            onChange={(e) =>
              update((d) => {
                d.advanced.logLevel = e.target.value as AppSettings['advanced']['logLevel'];
              })
            }
            className="h-8 w-40 rounded-md border border-strong bg-surface px-2 text-body text-primary"
          >
            <option value="error">Erros</option>
            <option value="warn">Avisos</option>
            <option value="info">Informações</option>
            <option value="debug">Depuração</option>
            <option value="trace">Tudo</option>
          </select>
          <span className="text-caption text-muted">Vale na próxima vez que o app abrir.</span>
        </div>
        <div>
          <Button onClick={() => void exportDiagnostics()}>Exportar diagnóstico</Button>
        </div>
      </Group>
      <Group title="Resetar">
        <p className="text-caption text-secondary">
          Volta todas as preferências ao padrão. Equipes, agentes, skills e segredos não são
          tocados.
        </p>
        <div className="flex gap-2">
          {confirmReset ? (
            <>
              <Button variant="danger" onClick={() => void reset()}>
                Resetar preferências
              </Button>
              <Button variant="ghost" onClick={() => setConfirmReset(false)}>
                Cancelar
              </Button>
            </>
          ) : (
            <Button onClick={() => setConfirmReset(true)}>Resetar…</Button>
          )}
        </div>
      </Group>
      {message && (
        <p
          role={message.tone === 'error' ? 'alert' : 'status'}
          className={cn('text-caption', message.tone === 'error' ? 'text-failed' : 'text-idle')}
        >
          {message.text}
        </p>
      )}
    </>
  );
}

/** Texto que grava ao sair do campo. */
function CommitInput({
  label,
  value,
  placeholder,
  hint,
  onCommit,
}: {
  label: string;
  value: string;
  placeholder?: string;
  hint?: string;
  onCommit: (value: string) => void;
}) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  const commit = () => {
    if (text.trim() !== value) onCommit(text.trim());
  };
  return (
    <Input
      label={label}
      value={text}
      placeholder={placeholder}
      hint={hint}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') commit();
      }}
    />
  );
}

/** Esqueleto no lugar de spinner (F08-03). */
function SettingsSkeleton(): ReactNode {
  return (
    <div
      className="flex h-full"
      role="status"
      aria-busy="true"
      aria-label="Carregando configurações"
    >
      <div className="w-48 shrink-0 space-y-2 border-r border-subtle p-3">
        {SECTIONS.map(([id]) => (
          <div
            key={id}
            className="h-6 animate-pulse rounded-md bg-hover motion-reduce:animate-none"
          />
        ))}
      </div>
      <div className="flex-1 space-y-3 p-6">
        <div className="h-5 w-40 animate-pulse rounded-md bg-hover motion-reduce:animate-none" />
        <div className="h-24 max-w-2xl animate-pulse rounded-md bg-hover motion-reduce:animate-none" />
      </div>
    </div>
  );
}
