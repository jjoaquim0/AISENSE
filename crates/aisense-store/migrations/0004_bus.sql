-- 0004 — barramento (F05-02, docs/07).
--
-- `messages` é recriada (estava vazia: nada gravava nela antes da Fase 05):
--   * `to_human`: mensagem para `@voce` é um quarto destino; o CHECK passa a exigir
--     exatamente um (invariante I2).
--   * `from_kind` diz quem mandou (agent | human | system) sem depender de NULL.
--   * `from_agent`, `to_agent` e `reply_to` sem chave estrangeira: a conversa sobrevive à
--     exclusão do agente (a linha do tempo mostra "agente removido"), e a retenção apaga
--     mensagens antigas sem esbarrar em respostas que apontam para elas.
-- `deliveries` é recriada junto só para acompanhar a troca da tabela que ela referencia.

CREATE TABLE messages_new (
  id          TEXT PRIMARY KEY,            -- ULID monotônico: a ordem do id é a do tempo
  team_id     TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL,               -- message | request | response | event | system
  from_kind   TEXT NOT NULL,               -- agent | human | system
  from_agent  TEXT,
  to_agent    TEXT,
  to_channel  TEXT REFERENCES channels(id) ON DELETE CASCADE,
  broadcast   INTEGER NOT NULL DEFAULT 0,
  to_human    INTEGER NOT NULL DEFAULT 0,
  reply_to    TEXT,
  subject     TEXT,
  body        TEXT NOT NULL,
  meta        TEXT NOT NULL DEFAULT '{}',
  created_at  INTEGER NOT NULL,
  CHECK ((to_agent IS NOT NULL) + (to_channel IS NOT NULL) + broadcast + to_human = 1)
);

INSERT INTO messages_new (id, team_id, kind, from_kind, from_agent, to_agent, to_channel,
                          broadcast, to_human, reply_to, subject, body, meta, created_at)
SELECT id, team_id, kind,
       CASE WHEN from_agent IS NOT NULL THEN 'agent' WHEN from_human = 1 THEN 'human' ELSE 'system' END,
       from_agent, to_agent, to_channel, broadcast, 0, reply_to, subject, body, meta, created_at
FROM messages;

CREATE TABLE deliveries_new (
  message_id   TEXT NOT NULL REFERENCES messages_new(id) ON DELETE CASCADE,
  agent_id     TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  state        TEXT NOT NULL DEFAULT 'pending', -- pending | delivered | read | failed | expired
  delivered_at INTEGER,
  read_at      INTEGER,
  attempts     INTEGER NOT NULL DEFAULT 0,
  error        TEXT,
  PRIMARY KEY (message_id, agent_id)
);

INSERT INTO deliveries_new SELECT message_id, agent_id, state, delivered_at, read_at, attempts, error
FROM deliveries;

DROP TABLE deliveries;
DROP TABLE messages;
ALTER TABLE messages_new RENAME TO messages;
ALTER TABLE deliveries_new RENAME TO deliveries;

CREATE INDEX idx_messages_team_id   ON messages(team_id, id);
CREATE INDEX idx_messages_reply_to  ON messages(reply_to);
CREATE INDEX idx_messages_created   ON messages(created_at);
CREATE INDEX idx_deliveries_agent   ON deliveries(agent_id, message_id);
CREATE INDEX idx_deliveries_pending ON deliveries(agent_id, state) WHERE state IN ('pending', 'delivered');
