-- 0002 — bancadas (docs/16-bancadas.md, "Esquema").

ALTER TABLE teams  ADD COLUMN workspace_mode TEXT NOT NULL DEFAULT 'shared'; -- shared|per-agent
ALTER TABLE teams  ADD COLUMN base_branch    TEXT;                            -- NULL = branch atual
ALTER TABLE agents ADD COLUMN workbench      TEXT NOT NULL DEFAULT 'inherit'; -- inherit|own|shared

CREATE TABLE workbenches (
  id         TEXT PRIMARY KEY,
  agent_id   TEXT NOT NULL UNIQUE REFERENCES agents(id) ON DELETE CASCADE,
  path       TEXT NOT NULL,
  branch     TEXT NOT NULL,
  base       TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
