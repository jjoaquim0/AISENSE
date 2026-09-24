/**
 * Core falso para os testes E2E do front (F08-08) e para screenshots sem janela.
 *
 * Só entra no bundle com `vite --mode e2e` (`main.tsx`). Responde aos comandos Tauri com
 * um estado em memória e emite os mesmos eventos que o core em Rust, pelo `mockIPC` do
 * `@tauri-apps/api/mocks`. O que ele **não** prova: PTY de verdade, barramento, skills no
 * boot, política de reinício — isso é coberto pelos testes de integração em Rust, com
 * processos reais (ver `docs/fases/FASE-08-acabamento.md`, F08-08).
 *
 * Os testes mexem no estado por `window.__fake` (ver `FakeHandle`).
 */
import { emit } from '@tauri-apps/api/event';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import type { Agent } from '@/types/generated/Agent';
import type { AgentDraft } from '@/types/generated/AgentDraft';
import type { AgentState } from '@/types/generated/AgentState';
import type { AppSettings } from '@/types/generated/AppSettings';
import type { Calibration } from '@/types/generated/Calibration';
import type { MessageView } from '@/types/generated/MessageView';
import type { PlannedAgent } from '@/types/generated/PlannedAgent';
import type { RuntimeInfo } from '@/types/generated/RuntimeInfo';
import type { StateRules } from '@/types/generated/StateRules';
import type { Team } from '@/types/generated/Team';
import type { TeamDraft } from '@/types/generated/TeamDraft';
import type { TeamSummary } from '@/types/generated/TeamSummary';
import type { TeamTemplate } from '@/types/generated/TeamTemplate';

/** Configuração inicial, passada pelo teste com `addInitScript` antes da página subir. */
export interface FakeSeed {
  onboardingDone?: boolean;
  /** Equipe já criada, com estes handles (runtime `shell`). */
  team?: { name: string; handles: string[]; running?: boolean };
  settings?: Partial<AppSettings>;
  /** Mensagens na linha do tempo da equipe semeada. */
  messages?: number;
  /**
   * Guarda o estado no `localStorage` e o recupera no próximo carregamento: um reload da
   * página faz o papel de fechar e reabrir o app (F08-06). Agentes voltam parados.
   */
  persist?: boolean;
  /**
   * Comandos que falham com um `CommandError` até o teste chamar `__fake.heal(cmd)`. Falhar
   * só uma vez não serve: em dev o StrictMode roda cada efeito duas vezes.
   */
  failing?: string[];
}

const STORE_KEY = 'aisense.fake-core';

export interface FakeHandle {
  emit: (event: string, payload: unknown) => Promise<void>;
  /** Muda o estado de um agente como o detector faria, com o evento. */
  setState: (handle: string, state: AgentState) => Promise<void>;
  /** Comandos que chegaram e o fake não conhece (para o teste falhar com o nome). */
  unknown: string[];
  calls: { cmd: string; args: unknown }[];
  settings: () => AppSettings;
  notifications: { title: string; body: string }[];
  /** Para de falhar `cmd` (ver `FakeSeed.failing`). */
  heal: (cmd: string) => void;
}

declare global {
  interface Window {
    __fakeSeed?: FakeSeed;
    __fake?: FakeHandle;
  }
}

const now = () => Date.now();
let seq = 0;
const id = (prefix: string) => `${prefix}_${(++seq).toString().padStart(6, '0')}`;
const b64 = (text: string) => btoa(String.fromCharCode(...new TextEncoder().encode(text)));

function defaultSettings(): AppSettings {
  return {
    version: 1,
    onboardingDone: false,
    appearance: {
      theme: 'system',
      density: 'comfortable',
      terminalFontSize: 13,
      terminalFontFamily: '',
    },
    session: { restoreLastTeam: true, relaunchAgents: false, lastTeam: null, runningAgents: [] },
    notifications: { enabled: true, awaitingInput: true, failed: true, mutedUntil: null },
    bus: {
      guards: { perAgentPerMinute: 30, maxReplyDepth: 12, maxIdentical: 3, teamPerHour: 500 },
      askDefaultSecs: 300,
      retentionDays: 90,
    },
    shortcuts: {},
    secrets: [],
    advanced: { logLevel: 'info' },
  };
}

const SHELL_RULES: StateRules = {
  idleRegex: '(?m)[$#%>]\\s*$',
  busyRegex: null,
  awaitingRegex: null,
  quietMs: 400,
};

function adapter(id: string, name: string, state: StateRules): RuntimeInfo['adapter'] {
  return {
    id,
    name,
    description: '',
    icon: null,
    command: id,
    args: [],
    detect: null,
    installHint: id === 'codex' ? 'npm i -g @openai/codex' : null,
    capabilities: {
      mcp: false,
      systemPromptFlag: null,
      hooks: false,
      modelFlag: null,
      cwdIsProject: false,
      resumeFlag: null,
      mcpConfig: null,
    },
    state,
    inject: { mode: 'stdin', submit: '\r', prefix: '[AISENSE] ', maxChars: 4000, boot: false },
    skills: null,
    env: {},
    source: { kind: 'builtin' },
  };
}

const RUNTIMES: RuntimeInfo[] = [
  {
    adapter: adapter('claude', 'Claude Code', {
      idleRegex: '(?m)^\\s*(?:│\\s*)?[>❯]\\s*$',
      busyRegex: '(?i)(thinking|esc to interrupt)',
      awaitingRegex: '(?i)(do you want|\\(y/n\\))',
      quietMs: 400,
    }),
    status: { status: 'available', path: '/usr/local/bin/claude', version: '2.1.0' },
  },
  {
    adapter: adapter('codex', 'Codex CLI', SHELL_RULES),
    status: { status: 'missing', reason: 'não encontrado no PATH' },
  },
  {
    adapter: adapter('shell', 'Shell', SHELL_RULES),
    status: { status: 'available', path: '/bin/bash', version: null },
  },
  { adapter: adapter('custom', 'Personalizado', SHELL_RULES), status: { status: 'perAgent' } },
];

const TEMPLATE_AGENTS: Record<TeamTemplate, [string, string, string][]> = {
  empty: [],
  'duo-dev': [
    ['dev', 'Dev', 'claude'],
    ['revisor', 'Revisor', 'codex'],
  ],
  'full-squad': [
    ['arquiteto', 'Arquiteto', 'claude'],
    ['backend', 'Backend', 'claude'],
    ['frontend', 'Frontend', 'claude'],
    ['revisor', 'Revisor', 'codex'],
  ],
  research: [['coordenador', 'Coordenador', 'claude']],
  operations: [['monitor', 'Monitor', 'shell']],
};

const COLORS = ['violet', 'cyan', 'emerald', 'amber', 'rose', 'indigo', 'teal', 'fuchsia'] as const;

function draftOf(handle: string, name: string, adapterId: string): AgentDraft {
  return {
    handle,
    name,
    role: '',
    adapterId,
    env: {},
    args: [],
    autostart: true,
    restartPolicy: 'on-crash',
    deliveryMode: 'pull',
    autonomy: 'ask',
    workbench: 'inherit',
  };
}

export function installFakeCore(): void {
  const seed = window.__fakeSeed ?? {};
  let settings: AppSettings = {
    ...defaultSettings(),
    ...seed.settings,
    onboardingDone: seed.onboardingDone ?? false,
  };
  const teams: Team[] = [];
  const agents: Agent[] = [];
  const states = new Map<string, AgentState>();
  const screens = new Map<string, string>();
  const handle: FakeHandle = {
    emit: (event, payload) => emit(event, payload),
    setState: async (h, state) => {
      const agent = agents.find((a) => a.handle === h);
      if (!agent) throw new Error(`agente @${h} não existe no fake`);
      await setState(agent.id, state);
    },
    unknown: [],
    calls: [],
    settings: () => settings,
    notifications: [],
    heal: (cmd) => failing.delete(cmd),
  };
  const failing = new Set(seed.failing ?? []);
  window.__fake = handle;

  const setState = async (agentId: string, state: AgentState) => {
    states.set(agentId, state);
    await emit('agent:state', { agentId, state, confidence: 'high' });
  };

  const write = async (agentId: string, text: string) => {
    screens.set(agentId, (screens.get(agentId) ?? '') + text);
    await emit('pty:data', { agentId, dataBase64: b64(text) });
  };

  const createTeam = (draft: TeamDraft, drafts: AgentDraft[]): Team => {
    const team: Team = {
      id: id('team'),
      name: draft.name,
      mission: draft.mission,
      workdir: draft.workdir,
      color: draft.color,
      icon: null,
      workspaceMode: draft.workspaceMode,
      layout: {},
      archivedAt: null,
      createdAt: now(),
      updatedAt: now(),
    };
    teams.push(team);
    drafts.forEach((d, i) => {
      const agent: Agent = {
        id: id('agent'),
        teamId: team.id,
        handle: d.handle,
        name: d.name,
        role: d.role,
        adapterId: d.adapterId,
        model: null,
        workdir: null,
        env: {},
        args: [],
        color: d.color ?? COLORS[i % COLORS.length] ?? 'violet',
        autostart: d.autostart,
        restartPolicy: d.restartPolicy,
        deliveryMode: d.deliveryMode,
        autonomy: d.autonomy,
        workbench: d.workbench,
        position: i,
        createdAt: now(),
        updatedAt: now(),
      };
      agents.push(agent);
      states.set(agent.id, 'stopped');
    });
    return team;
  };

  const startAgent = async (agentId: string) => {
    await setState(agentId, 'starting');
    screens.set(agentId, '');
    setTimeout(() => {
      void write(agentId, `bash-5.2$ `).then(() => setState(agentId, 'idle'));
    }, 30);
    return {
      workdir: { path: teams[0]?.workdir ?? '/tmp', bench: null, warning: null },
      skills: { active: [], ignored: [] },
      notes: [],
      boot: { channel: { kind: 'none' }, status: 'skipped', message: 'shell' },
    };
  };

  const summaries = (includeArchived: boolean): TeamSummary[] =>
    teams
      .filter((t) => includeArchived || !t.archivedAt)
      .map((team) => ({
        team,
        agents: agents
          .filter((a) => a.teamId === team.id)
          .map((a) => ({
            id: a.id,
            handle: a.handle,
            name: a.name,
            color: a.color,
            adapterId: a.adapterId,
            autostart: a.autostart,
            state: states.get(a.id) ?? 'stopped',
          })),
      }));

  const saveState = () => {
    if (!seed.persist) return;
    localStorage.setItem(STORE_KEY, JSON.stringify({ settings, teams, agents, seq }));
  };
  const savedState = seed.persist ? localStorage.getItem(STORE_KEY) : null;
  if (savedState) {
    const parsed = JSON.parse(savedState) as {
      settings: AppSettings;
      teams: Team[];
      agents: Agent[];
      seq: number;
    };
    settings = parsed.settings;
    teams.push(...parsed.teams);
    agents.push(...parsed.agents);
    seq = parsed.seq;
    for (const a of agents) states.set(a.id, 'stopped');
  }

  if (seed.team && !savedState) {
    const team = createTeam(
      {
        name: seed.team.name,
        mission: '',
        workdir: '/home/voce/projetos/api',
        color: 'violet',
        workspaceMode: 'shared',
      },
      seed.team.handles.map((h) => draftOf(h, h[0]?.toUpperCase() + h.slice(1), 'shell')),
    );
    if (seed.team.running) {
      for (const a of agents.filter((x) => x.teamId === team.id)) {
        states.set(a.id, 'idle');
        screens.set(a.id, `bash-5.2$ echo pronto\r\npronto\r\nbash-5.2$ `);
      }
    }
  }

  const messages: MessageView[] = Array.from({ length: seed.messages ?? 0 }, (_, i) => ({
    id: `msg_${String(i).padStart(6, '0')}`,
    kind: 'message',
    from: i % 2 ? '@frontend' : '@backend',
    to: i % 2 ? '@backend' : '@frontend',
    subject: null,
    body: `Mensagem ${i + 1}: ${'detalhe do trabalho '.repeat((i % 4) + 1)}`,
    replyTo: null,
    meta: { priority: 'normal', attachments: [] },
    createdAt: 1_700_000_000_000 + i * 60_000,
    receipts: { recipients: 1, delivered: 1, read: 1, failed: 0 },
  }));

  const calibrate = (rules: StateRules, screen: string): Calibration => {
    const lines = screen
      .split('\n')
      .map((l) => l.trimEnd())
      .filter((l) => l.trim())
      .slice(-12);
    const fields: [string, string | null][] = [
      ['awaiting_regex', rules.awaitingRegex],
      ['busy_regex', rules.busyRegex],
      ['idle_regex', rules.idleRegex],
    ];
    const patterns = fields.map(([field, pattern]) => {
      if (!pattern) return { field, error: null, matches: [] as number[] };
      try {
        const re = new RegExp(pattern.replace(/^\(\?[a-z]+\)/, ''), 'm');
        return { field, error: null, matches: lines.flatMap((l, i) => (re.test(l) ? [i] : [])) };
      } catch (e) {
        return { field, error: String(e), matches: [] as number[] };
      }
    });
    let decided: Calibration['decided'] = null;
    let decidedLine: number | null = null;
    for (let i = lines.length - 1; i >= 0 && decided === null; i--) {
      for (const p of patterns) {
        if (p.matches.includes(i)) {
          decided =
            p.field === 'idle_regex'
              ? 'idle'
              : p.field === 'busy_regex'
                ? 'busy'
                : 'awaiting_input';
          decidedLine = i;
          break;
        }
      }
    }
    return { lines, decided, decidedLine, patterns };
  };

  // biome-ignore lint/suspicious/noExplicitAny: argumentos chegam como JSON solto do invoke
  const handlers: Record<string, (a: any) => unknown> = {
    app_info: () => ({ version: '0.1.0', platform: 'linux', debug: true }),
    runtimes_overview: () => ({ runtimes: RUNTIMES, problems: [] }),
    settings_get: () => ({
      settings,
      dataDir: '/home/voce/.aisense',
      settingsPath: '/home/voce/.aisense/settings.json',
      adaptersDir: '/home/voce/.aisense/adapters',
      warning: null,
    }),
    settings_save: ({ next }) => {
      settings = {
        ...next,
        secrets: settings.secrets,
        session: { ...next.session, lastTeam: settings.session.lastTeam },
      };
      void emit('settings:changed', settings);
      return settings;
    },
    settings_reset: () => {
      settings = { ...defaultSettings(), onboardingDone: true, secrets: settings.secrets };
      return settings;
    },
    settings_onboarding_done: () => {
      settings = { ...settings, onboardingDone: true };
      return settings;
    },
    ui_viewing: () => null,
    settings_last_team: ({ teamId }) => {
      settings = { ...settings, session: { ...settings.session, lastTeam: teamId } };
      return null;
    },
    secret_set: ({ adapterId, envName, value }) => {
      settings = {
        ...settings,
        secrets: [
          ...settings.secrets,
          { adapterId, envName, masked: `${String(value).slice(0, 3)}…${String(value).slice(-4)}` },
        ],
      };
      return settings;
    },
    secret_delete: ({ adapterId, envName }) => {
      settings = {
        ...settings,
        secrets: settings.secrets.filter(
          (s) => !(s.adapterId === adapterId && s.envName === envName),
        ),
      };
      return settings;
    },
    calibration_screen: ({ agentId }) => (screens.get(agentId) ?? '').replace(/\r/g, ''),
    calibration_test: ({ rules, screen }) => calibrate(rules, screen),
    calibration_apply: ({ adapterId }) => `/home/voce/.aisense/adapters/${adapterId}.toml`,
    diagnostics_export: () => '/home/voce/.aisense/logs/diagnostico.json',
    teams_list: ({ includeArchived }) => summaries(Boolean(includeArchived)),
    team_template_plan: ({ template }): PlannedAgent[] =>
      (TEMPLATE_AGENTS[template as TeamTemplate] ?? []).map(([h, name, adapterId]) => ({
        draft: draftOf(h, name, adapterId),
        preferredAdapterId: adapterId,
        unavailable: adapterId === 'codex',
      })),
    team_create: ({ team, agents: drafts }) => createTeam(team, drafts),
    team_set_layout: ({ teamId, layout }) => {
      const team = teams.find((t) => t.id === teamId);
      if (team) team.layout = layout;
      return null;
    },
    team_set_archived: () => null,
    team_delete: () => null,
    team_start: async ({ teamId }) => {
      const list = agents.filter((a) => a.teamId === teamId && a.autostart);
      let done = 0;
      for (const a of list) {
        await startAgent(a.id);
        done++;
        await emit('team:progress', {
          teamId,
          op: 'start',
          done,
          total: list.length,
          agentId: a.id,
          finished: done === list.length,
        });
      }
      return { failures: [], notices: [] };
    },
    team_stop: async ({ teamId }) => {
      for (const a of agents.filter((x) => x.teamId === teamId)) await setState(a.id, 'stopped');
      return null;
    },
    team_restart: () => ({ failures: [], notices: [] }),
    agents_list: ({ teamId }) => agents.filter((a) => a.teamId === teamId),
    agent_state: ({ agentId }) => states.get(agentId) ?? 'stopped',
    agent_start: ({ agentId }) => startAgent(agentId),
    agent_restart: ({ agentId }) => startAgent(agentId),
    agent_stop: async ({ agentId }) => {
      await setState(agentId, 'stopped');
      return null;
    },
    agent_boot: () => null,
    agent_previews: ({ agentIds }) =>
      (agentIds as string[]).map((agentId) => ({
        agentId,
        lines: (screens.get(agentId) ?? '')
          .replace(/\r/g, '')
          .split('\n')
          .filter(Boolean)
          .slice(-5),
      })),
    agent_sessions: () => [],
    agent_skills_get: () => [],
    agent_skills_plan: () => ({ active: [], ignored: [] }),
    handle_suggest: () => null,
    pty_show: ({ agentId }) => {
      const screen = screens.get(agentId);
      return screen ? b64(screen) : null;
    },
    pty_write: async ({ agentId, data }) => {
      const text = String(data);
      const echo = text.replace(/\r/g, '\r\n');
      await write(agentId, text.endsWith('\r') ? `${echo}bash-5.2$ ` : echo);
      return null;
    },
    pty_is_running: ({ agentId }) => (states.get(agentId) ?? 'stopped') !== 'stopped',
    pty_resize: () => null,
    pty_set_visible: () => null,
    pty_clear: () => null,
    pty_snapshot: () => '',
    skills_library: () => ({ skills: [], problems: [] }),
    // Mais nova primeiro, antes do cursor (como o core).
    bus_timeline: ({ before, limit }) => {
      const end = before ? messages.findIndex((m) => m.id === before) : messages.length;
      return messages.slice(Math.max(0, end - limit), end).reverse();
    },
    bus_unread: () => [],
    board_get: ({ teamId }) => {
      const team = teams.find((t) => t.id === teamId);
      const boardId = `board_${teamId}`;
      const spec: [string, string, string][] = [
        ['backlog', 'Backlog', 'intake'],
        ['todo', 'A fazer', 'ready'],
        ['doing', 'Fazendo', 'active'],
        ['blocked', 'Bloqueada', 'blocked'],
        ['review', 'Revisão', 'review'],
        ['done', 'Feita', 'terminal'],
      ];
      return {
        teamId,
        teamName: team?.name ?? '',
        board: { id: boardId, teamId, automations: [], createdAt: 0 },
        columns: spec.map(([slug, name, kind], i) => ({
          id: `col_${slug}`,
          boardId,
          slug,
          name,
          kind,
          wipLimit: null,
          wipPerAgent: slug === 'doing' ? 1 : null,
          position: i,
          requiresApproval: false,
          approverMustDiffer: false,
          requiresCommands: [],
        })),
        cards: [],
        agents: agents
          .filter((a) => a.teamId === teamId)
          .map((a) => ({ id: a.id, handle: a.handle, color: a.color })),
        now: now(),
      };
    },
    board_changes: () => [],
    bus_paused: () => false,
    channels_list: () => [],
    proposals_list: () => [],
    notes_list: () => [],
    project_lookup: () => ({ status: 'missing', proposal: null }),
    'plugin:notification|is_permission_granted': () => true,
    'plugin:notification|request_permission': () => 'granted',
    'plugin:notification|notify': ({ options }) => {
      handle.notifications.push({ title: options?.title ?? '', body: options?.body ?? '' });
      return null;
    },
  };

  mockWindows('main');
  mockIPC(
    (cmd, args) => {
      handle.calls.push({ cmd, args });
      if (failing.has(cmd)) {
        throw {
          code: 'fake_failure',
          message: `could not run ${cmd}: simulated failure`,
          hint: 'Tente de novo.',
        };
      }
      const run = handlers[cmd];
      if (run) {
        const result = run(args ?? {});
        saveState();
        return result;
      }
      if (!cmd.startsWith('plugin:')) handle.unknown.push(cmd);
      return null;
    },
    { shouldMockEvents: true },
  );
}
