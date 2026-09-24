# 04 — Modelo de Dados

## Entidades

```
Workspace (implícito, 1 por instalação)
 └── Team ────────┬── Agent ──┬── AgentSkill ──► Skill
                  │           ├── PtySession (runtime, não persistido integralmente)
                  │           └── Message (from/to)
                  ├── Channel ── Message
                  └── Task ──── atribuída a Agent
```

## Esquema SQLite

> Migrações em `crates/aisense-store/migrations/NNNN_descricao.sql`, aplicadas na subida do app.
> `0001_initial.sql` é este esquema; `0002_workbenches.sql` é o adendo de [16 — Bancadas](16-bancadas.md).
> Os dois `PRAGMA` abaixo **não** ficam na migração (dentro de transação o SQLite os ignora): o
> `Store` os aplica em toda conexão, junto com `synchronous = NORMAL` e `busy_timeout = 5 s`.
> Antes de aplicar migração pendente num banco que já tem dados, o `Store` grava uma cópia em
> `aisense.db.bak-v<versão atual>`.
> IDs são **ULID** em texto (26 chars): ordenáveis por tempo, bons para índice de mensagens.
> Timestamps são `INTEGER` em epoch **milissegundos UTC**. Nunca guarde hora local.

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

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
  position      INTEGER NOT NULL DEFAULT 0,         -- ordem da equipe: sidebar e ▶ (`agents_reorder`)
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  UNIQUE (team_id, handle)
);
CREATE INDEX idx_agents_team ON agents(team_id);

-- ─────────────────────────────── SKILLS ────────────────────────────────
-- Espelho da biblioteca do disco (F04-02): o conteúdo fica no SKILL.md; aqui, a identidade
-- que agent_skills referencia. Casada por slug a cada recarga; o que some do disco NÃO é
-- apagado (um SKILL.md quebrado não leva as atribuições junto).
CREATE TABLE skills (
  id          TEXT PRIMARY KEY,
  slug        TEXT NOT NULL UNIQUE,        -- 'revisor-rigoroso'
  name        TEXT NOT NULL,               -- hoje igual ao slug
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

-- 0006 (F07-05): inscritos. Canal sem linhas aqui é aberto (vai para a equipe toda).
CREATE TABLE channel_members (
  channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
  agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  PRIMARY KEY (channel_id, agent_id)
);

CREATE TABLE messages (                  -- recriada na migração 0004 (F05-02)
  id          TEXT PRIMARY KEY,            -- ULID monotônico: a ordem do id é a do tempo
  team_id     TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL,               -- message | request | response | event | system
  from_kind   TEXT NOT NULL,               -- agent | human | system
  from_agent  TEXT,                        -- sem FK: a conversa sobrevive ao agente excluído
  to_agent    TEXT,                        -- DM (sem FK, pelo mesmo motivo)
  to_channel  TEXT REFERENCES channels(id) ON DELETE CASCADE, -- ou canal
  broadcast   INTEGER NOT NULL DEFAULT 0,                     -- ou toda a equipe (@all)
  to_human    INTEGER NOT NULL DEFAULT 0,                     -- ou o humano (@voce)
  reply_to    TEXT,                        -- sem FK: a retenção apaga a pergunta antes da resposta
  subject     TEXT,
  body        TEXT NOT NULL,
  meta        TEXT NOT NULL DEFAULT '{}',  -- JSON: prioridade, anexos, timeout
  created_at  INTEGER NOT NULL,
  CHECK ((to_agent IS NOT NULL) + (to_channel IS NOT NULL) + broadcast + to_human = 1)
);
CREATE INDEX idx_messages_team_id  ON messages(team_id, id);      -- linha do tempo por cursor
CREATE INDEX idx_messages_reply_to ON messages(reply_to);
CREATE INDEX idx_messages_created  ON messages(created_at);       -- retenção

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
CREATE INDEX idx_deliveries_agent   ON deliveries(agent_id, message_id);   -- caixa de entrada
CREATE INDEX idx_deliveries_pending ON deliveries(agent_id, state) WHERE state IN ('pending', 'delivered');

-- ───────────────────────────── TAREFAS ─────────────────────────────────
-- Os cartões do quadro. A migração 0005 (Fase 06) acrescenta column_id, priority, labels,
-- checklist, links, block_reason, version, archived_at, approved_by/at e column_since, e as
-- tabelas boards, columns, task_dependencies, task_comments e task_activity: ver o esquema
-- completo em 13-quadro-kanban.md. `status` fica só para as tarefas de antes do quadro.
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
  log_path   TEXT NOT NULL,
  log_offset INTEGER         -- bytes do log no início da sessão (0003); a sessão vai até a seguinte
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
```

## `teams.layout`

JSON da interface, gravado por `team_set_layout` (objeto, até 64 KB). Cada vista guarda a sua chave
e preserva as outras:

```jsonc
{
  "view": "grid",                  // "grid" | "focus" (Fluxo e Timeline chegam nas Fases 5 e 7)
  "grid": {
    "preset": "4",                 // "1" | "2" | "3" | "4" | "6" | "9" | "free"
    "order": ["agt_…", "agt_…"],   // nos presets, só os N primeiros aparecem
    "free": { "agt_…": { "x": 0, "y": 0, "w": 8, "h": 8, "z": 3 } }  // grade 24×24
  }
}
```

Layout salvo inválido nunca quebra a tela: a vista cai para o padrão.

## Invariantes

| # | Invariante | Garantido por |
|---|---|---|
| I1 | `handle` é único dentro da equipe e casa com `^[a-z][a-z0-9-]{1,31}$` | `UNIQUE` + validação no core |
| I2 | Uma mensagem tem exatamente um destino (agente, canal, broadcast ou humano) | `CHECK` |
| I3 | Toda mensagem entregue tem uma linha em `deliveries` por destinatário | Transação única no `Bus::route` |
| I4 | Token de IPC só é válido enquanto a sessão está viva | `expires_at` + limpeza no `session end` |
| I5 | Apagar equipe apaga agentes, canais, mensagens e tarefas | `ON DELETE CASCADE` |
| I6 | `reply_to` só aponta para mensagem da mesma equipe | Validação no core (SQLite não expressa) |
| I7 | `agents.env` nunca define variável com prefixo `AISENSE_` (seria possível se passar por outro agente) nem nome fora de `[A-Za-z_][A-Za-z0-9_]*` | Validação no core (`AgentDraft`) |
| I8 | `handle` não é `all` nem `voce` — o barramento usa esses endereços para a equipe e o humano | Validação no core (`Handle::parse`) |

## Retenção

| Dado | Retenção padrão | Configurável |
|---|---|---|
| `messages` | 90 dias | Sim (`config.toml`) |
| `deliveries` | Junto com a mensagem | — |
| Logs de PTY em arquivo | 30 dias ou 200 MB por agente, o que vier primeiro | Sim |
| Ring buffer em RAM | 10.000 linhas por agente | Sim |
| `sessions` | 200 últimas por agente | Sim |

Limpeza roda no start do app e a cada 6 h, em transação, fora do caminho crítico.

## Tipos compartilhados com o front

Os modelos de domínio (`Team`, `Agent`, `TeamDraft`, `AgentDraft`) serializam em **camelCase**
(`teamId`, `adapterId`...), que é o que o front espera; o esquema SQL continua em snake_case e a
tradução é do store. Timestamps viram `number` no TypeScript (epoch ms cabe com folga em 2^53).

Todo struct que cruza a fronteira Tauri é anotado com `#[derive(TS)]` (`ts-rs`) e exportado para
`apps/desktop/src/types/generated/`. **Não escreva esses tipos à mão no TypeScript** — rode
`cargo test -p aisense-core export_bindings` para regenerar. Divergência aqui é fonte garantida de bug.
