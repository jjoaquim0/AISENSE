-- 0007 — propostas de ações estruturais (F07-02, docs/11).
-- Um agente propõe; só o humano aceita (e o AISENSE executa) ou recusa.
CREATE TABLE proposals (
  id            TEXT PRIMARY KEY,
  team_id       TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  proposed_by   TEXT REFERENCES agents(id) ON DELETE SET NULL,
  action        TEXT NOT NULL,              -- JSON (ProposalAction)
  reason        TEXT NOT NULL,
  state         TEXT NOT NULL DEFAULT 'pending',  -- pending | accepted | rejected
  created_at    INTEGER NOT NULL,
  decided_at    INTEGER,
  decision_note TEXT
);
CREATE INDEX idx_proposals_team ON proposals(team_id, state);
