-- 0001 — esquema inicial (docs/04-modelo-de-dados.md, copiado sem alteração).
--
-- Os PRAGMAs do documento (journal_mode = WAL, foreign_keys = ON) NÃO estão aqui:
-- migrações rodam dentro de transação, onde os dois são ignorados em silêncio.
-- Eles são aplicados em toda conexão pelo `Store` (src/db.rs).
--
-- Migrações são só aditivas. Nunca edite um arquivo já publicado: o checksum dele está
-- gravado no banco do usuário e a subida falha. Mudou de ideia? Crie a próxima.

-- ─────────────────────────────── EQUIPES ───────────────────────────────
CREATE TABLE teams (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  mission      TEXT NOT NULL DEFAULT '',   -- injetada no bootstrap de todos os agentes
  workdir      TEXT NOT NULL,              -- diretório de trabalho padrão
  color        TEXT NOT NULL DEFAULT 'violet',
  icon         TEXT,
  layout       TEXT NOT NULL DEFAULT '{}', -- JSON: modo de vista, posição dos painéis
  archived_at  INTEGER,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);

-- ─────────────────────────────── AGENTES ───────────────────────────────
CREATE TABLE agents (
  id            TEXT PRIMARY KEY,
  team_id       TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  handle        TEXT NOT NULL,              -- endereço no barramento, sem '@'. Ex: 'backend'
  name          TEXT NOT NULL,              -- rótulo de exibição. Ex: 'Backend'
  role          TEXT NOT NULL DEFAULT '',   -- descrição do papel, vai no bootstrap
  adapter_id    TEXT NOT NULL,              -- 'claude' | 'codex' | 'opencode' | 'shell' | ...
  model         TEXT,                       -- opcional, repassado ao adaptador
  workdir       TEXT,                       -- NULL = herda da equipe
  env           TEXT NOT NULL DEFAULT '{}', -- JSON de variáveis extras
  args          TEXT NOT NULL DEFAULT '[]', -- JSON de argumentos extras da CLI
  color         TEXT NOT NULL,              -- cor de identidade (borda, bolhas, nó no canvas)
  autostart     INTEGER NOT NULL DEFAULT 0,
  restart_policy TEXT NOT NULL DEFAULT 'on-crash',  -- never | on-crash | always
  delivery_mode TEXT NOT NULL DEFAULT 'pull',       -- pull | push | hook   (ver docs/07)
  autonomy      TEXT NOT NULL DEFAULT 'ask',        -- ask | trusted        (ver docs/11)
  position      INTEGER NOT NULL DEFAULT 0,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  UNIQUE (team_id, handle)
);
CREATE INDEX idx_agents_team ON agents(team_id);

-- ─────────────────────────────── SKILLS ────────────────────────────────
CREATE TABLE skills (
  id          TEXT PRIMARY KEY,
  slug        TEXT NOT NULL UNIQUE,        -- 'revisor-rigoroso'
  name        TEXT NOT NULL,
  description TEXT NOT NULL,
  version     TEXT NOT NULL DEFAULT '1.0.0',
  source      TEXT NOT NULL,               -- builtin | user | imported
  path        TEXT NOT NULL,               -- diretório que contém SKILL.md
  targets     TEXT NOT NULL DEFAULT '[]',  -- JSON: adapter_ids compatíveis; [] = todos
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE agent_skills (
  agent_id  TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  skill_id  TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  position  INTEGER NOT NULL DEFAULT 0,    -- ordem de injeção no bootstrap
  enabled   INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (agent_id, skill_id)
);

-- ──────────────────────────── COMUNICAÇÃO ──────────────────────────────
CREATE TABLE channels (
  id         TEXT PRIMARY KEY,
  team_id    TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  slug       TEXT NOT NULL,                -- 'geral', sem '#'
  topic      TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  UNIQUE (team_id, slug)
);

CREATE TABLE messages (
  id          TEXT PRIMARY KEY,            -- ULID
  team_id     TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL,               -- message | request | response | event | system
  from_agent  TEXT REFERENCES agents(id) ON DELETE SET NULL,  -- NULL = humano ou sistema
  from_human  INTEGER NOT NULL DEFAULT 0,
  to_agent    TEXT REFERENCES agents(id) ON DELETE SET NULL,  -- DM
  to_channel  TEXT REFERENCES channels(id) ON DELETE CASCADE, -- ou canal
  broadcast   INTEGER NOT NULL DEFAULT 0,                     -- ou toda a equipe
  reply_to    TEXT REFERENCES messages(id) ON DELETE SET NULL,
  subject     TEXT,
  body        TEXT NOT NULL,
  meta        TEXT NOT NULL DEFAULT '{}',  -- JSON: prioridade, anexos, timeout
  created_at  INTEGER NOT NULL,
  CHECK (to_agent IS NOT NULL OR to_channel IS NOT NULL OR broadcast = 1)
);
CREATE INDEX idx_messages_team_time ON messages(team_id, created_at DESC);
CREATE INDEX idx_messages_to_agent  ON messages(to_agent, created_at DESC);
CREATE INDEX idx_messages_reply_to  ON messages(reply_to);

-- Estado de entrega por destinatário: uma mensagem broadcast tem N linhas aqui.
CREATE TABLE deliveries (
  message_id   TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  agent_id     TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  state        TEXT NOT NULL DEFAULT 'pending', -- pending | delivered | read | failed | expired
  delivered_at INTEGER,
  read_at      INTEGER,
  attempts     INTEGER NOT NULL DEFAULT 0,
  error        TEXT,
  PRIMARY KEY (message_id, agent_id)
);
CREATE INDEX idx_deliveries_pending ON deliveries(agent_id, state) WHERE state = 'pending';

-- ───────────────────────────── TAREFAS ─────────────────────────────────
CREATE TABLE tasks (
  id          TEXT PRIMARY KEY,
  team_id     TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  title       TEXT NOT NULL,
  body        TEXT NOT NULL DEFAULT '',
  status      TEXT NOT NULL DEFAULT 'todo',  -- todo | doing | blocked | review | done | cancelled
  assignee    TEXT REFERENCES agents(id) ON DELETE SET NULL,
  created_by  TEXT REFERENCES agents(id) ON DELETE SET NULL,
  parent_id   TEXT REFERENCES tasks(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX idx_tasks_team_status ON tasks(team_id, status);

-- ───────────────────────────── SESSÕES ─────────────────────────────────
-- Histórico de execuções de PTY. O buffer vivo NÃO fica aqui (fica em RAM + log em arquivo).
CREATE TABLE sessions (
  id         TEXT PRIMARY KEY,
  agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  pid        INTEGER,
  started_at INTEGER NOT NULL,
  ended_at   INTEGER,
  exit_code  INTEGER,
  log_path   TEXT NOT NULL
);
CREATE INDEX idx_sessions_agent ON sessions(agent_id, started_at DESC);

-- ─────────────────────────── AUTENTICAÇÃO IPC ──────────────────────────
-- Token efêmero por sessão de agente. Apagado quando a sessão termina.
CREATE TABLE agent_tokens (
  token      TEXT PRIMARY KEY,
  agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  expires_at INTEGER NOT NULL
);
