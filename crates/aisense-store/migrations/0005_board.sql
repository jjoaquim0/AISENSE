-- 0005 — quadro Kanban (F06-02, docs/13 "Esquema").
--
-- Além do adendo de docs/13:
--   * `columns.wip_per_agent`: o "1 por agente" de Fazendo não cabe em `wip_limit` (total).
--   * `tasks.column_since`: desde quando o cartão está na coluna (`card_stale`, "há 18min").
--   * `author_kind` em comentários e histórico: distingue você (humano) do próprio AISENSE
--     (automação, gate) — os dois têm `author` NULL.
-- Equipes que já existiam ganham o quadro na primeira leitura (`board::ensure_board`), que
-- também põe numa coluna as tarefas antigas (pela `status`).

CREATE TABLE boards (
  id          TEXT PRIMARY KEY,
  team_id     TEXT NOT NULL UNIQUE REFERENCES teams(id) ON DELETE CASCADE,
  automations TEXT NOT NULL DEFAULT '[]',        -- JSON
  created_at  INTEGER NOT NULL
);

CREATE TABLE columns (
  id                   TEXT PRIMARY KEY,
  board_id             TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  slug                 TEXT NOT NULL,
  name                 TEXT NOT NULL,
  kind                 TEXT NOT NULL,             -- intake|ready|active|blocked|review|terminal
  wip_limit            INTEGER,                   -- NULL = sem limite
  wip_per_agent        INTEGER,                   -- NULL = sem limite por agente
  position             INTEGER NOT NULL,
  requires_approval    INTEGER NOT NULL DEFAULT 0,
  approver_must_differ INTEGER NOT NULL DEFAULT 1,
  requires_commands    TEXT NOT NULL DEFAULT '[]',
  UNIQUE (board_id, slug)
);

-- Coluna com cartão não some por engano: sem ON DELETE, a FK recusa (o editor move antes).
ALTER TABLE tasks ADD COLUMN column_id    TEXT REFERENCES columns(id);
ALTER TABLE tasks ADD COLUMN priority     TEXT NOT NULL DEFAULT 'normal'; -- low|normal|high|urgent
ALTER TABLE tasks ADD COLUMN labels       TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN checklist    TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN links        TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN block_reason TEXT;
ALTER TABLE tasks ADD COLUMN version      INTEGER NOT NULL DEFAULT 1;     -- trava otimista do claim
ALTER TABLE tasks ADD COLUMN archived_at  INTEGER;
ALTER TABLE tasks ADD COLUMN approved_by  TEXT REFERENCES agents(id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN approved_at  INTEGER;
ALTER TABLE tasks ADD COLUMN column_since INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_tasks_column ON tasks(column_id, position) WHERE archived_at IS NULL;
CREATE INDEX idx_tasks_assignee ON tasks(assignee) WHERE archived_at IS NULL;

CREATE TABLE task_dependencies (
  task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  depends_on TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, depends_on),
  CHECK (task_id <> depends_on)
);
CREATE INDEX idx_dependencies_on ON task_dependencies(depends_on);

CREATE TABLE task_comments (
  id          TEXT PRIMARY KEY,
  task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author_kind TEXT NOT NULL DEFAULT 'human',      -- human | agent | system
  author      TEXT REFERENCES agents(id) ON DELETE SET NULL,
  body        TEXT NOT NULL,
  created_at  INTEGER NOT NULL
);
CREATE INDEX idx_comments_task ON task_comments(task_id, created_at);

CREATE TABLE task_activity (
  id          TEXT PRIMARY KEY,
  task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author_kind TEXT NOT NULL DEFAULT 'human',
  author      TEXT REFERENCES agents(id) ON DELETE SET NULL,
  action      TEXT NOT NULL,                      -- created|moved|assigned|commented|...
  detail      TEXT NOT NULL DEFAULT '{}',
  created_at  INTEGER NOT NULL
);
CREATE INDEX idx_activity_task ON task_activity(task_id, created_at);
