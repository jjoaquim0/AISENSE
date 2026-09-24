-- 0006 — inscrição em canais (F07-05, docs/07 "Endereçamento").
-- Canal sem inscritos é aberto: a mensagem vai para a equipe toda (como antes).
-- Com inscritos, só eles recebem.
CREATE TABLE channel_members (
  channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
  agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  PRIMARY KEY (channel_id, agent_id)
);
CREATE INDEX idx_channel_members_agent ON channel_members(agent_id);
