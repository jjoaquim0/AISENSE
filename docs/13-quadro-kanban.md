# 13 — Quadro Kanban

> **Cada equipe tem um quadro. O quadro é a memória compartilhada da equipe.**
> É onde o trabalho vira estado explícito — visível para você e para todos os agentes ao mesmo tempo.
> Agentes criam, leem, atualizam e concluem cartões pelas mesmas operações que você usa na interface.

## Por que o quadro é central (e não um acessório)

Sem quadro, a coordenação vive dentro da janela de contexto de cada agente — some quando o terminal
reinicia e ninguém mais enxerga. Com quadro:

| Problema | O quadro resolve |
|---|---|
| Dois agentes fazendo a mesma coisa | `claim` atômico: só um pega o cartão |
| "Em que pé está aquilo?" | `aisense board` responde em texto, para o humano e para a IA |
| Agente reiniciou e perdeu o fio | O cartão continua lá, com histórico e comentários |
| Trabalho que depende de outro | Dependências explícitas (`--blocked-by`) |
| Coordenador virando gargalo | O agente puxa trabalho da coluna em vez de esperar ser mandado |
| Você voltou depois de 2 horas | O quadro mostra o que mudou, sem ler 9 terminais |

O quadro é criado **automaticamente** junto com a equipe. Não existe equipe sem quadro.

## Modelo

```
Board (1 por equipe)
 ├── Column (ordenadas, configuráveis, com limite de WIP)
 ├── Card
 │    ├── checklist[]         itens marcáveis
 │    ├── comment[]           thread — agentes comentam, você comenta
 │    ├── label[]
 │    ├── dependency[]        bloqueia / bloqueado por
 │    ├── subtask[]           cartões filhos
 │    ├── link[]              PR, commit, arquivo, URL
 │    └── activity[]          log imutável de tudo que aconteceu
 └── Automation[]             regras declarativas por coluna
```

### Colunas padrão

| Slug | Nome | Limite WIP | Semântica |
|---|---|---|---|
| `backlog` | Backlog | — | Ideias e trabalho ainda não priorizado |
| `todo` | A fazer | — | Pronto para alguém pegar. **É daqui que o agente puxa trabalho** |
| `doing` | Fazendo | 1 por agente | Em execução. WIP limitado evita agente picando tarefa |
| `blocked` | Bloqueada | — | Requer algo de fora. Exige motivo obrigatório |
| `review` | Revisão | — | Feito, aguardando outro agente ou você |
| `done` | Feita | — | Concluída |

Colunas são configuráveis por equipe (criar, renomear, reordenar, remover, mudar WIP), mas toda
coluna tem um **tipo semântico** (`intake · ready · active · blocked · review · terminal`) para que
as automações e os agentes saibam o que ela significa sem depender do nome.

## API dos agentes

Disponível na CLI `aisense` (qualquer terminal) e como ferramentas MCP (runtimes compatíveis).
Toda operação aceita `--json`.

### Leitura

```bash
aisense board                       # quadro inteiro em ASCII, otimizado para LLM ler
aisense board --column doing        # só uma coluna
aisense task list --mine            # meus cartões
aisense task list --column todo --unassigned --label backend
aisense task show tsk_01J8X         # cartão completo: corpo, checklist, comentários, histórico
aisense task next                   # sugere o próximo cartão que EU deveria pegar
```

`aisense board` imprime algo que um modelo entende de primeira:

```
QUADRO — Squad Produto                          atualizado há 2min

A FAZER (4)
  tsk_7K2  [alta]  Migrar /users para OAuth              sem responsável
  tsk_7K3  [média] Atualizar o cliente TypeScript        bloqueado por tsk_7K2
  ...
FAZENDO (2/4)
  tsk_7J9  [alta]  Middleware de refresh token           @backend   há 18min
  ...
BLOQUEADA (1)
  tsk_7H1  Remover legacy_id          motivo: aguarda decisão do @arquiteto
REVISÃO (1)
  tsk_7J4  Endpoint /auth/token       @backend → aguarda @revisor   há 40min
```

### Escrita

```bash
aisense task add "Migrar /users para OAuth" \
  --body "Manter compatibilidade com o cliente antigo por 2 versões." \
  --assign @backend --column todo --label backend --priority high \
  --blocked-by tsk_7K1 --checklist "escrever teste,implementar,atualizar doc"

aisense task claim tsk_7K2                  # pega para si (ATÔMICO — ver abaixo)
aisense task move tsk_7K2 doing
aisense task update tsk_7K2 --assign @frontend --add-label urgente
aisense task check tsk_7K2 1                # marca o item 1 do checklist
aisense task comment tsk_7K2 "o contrato mudou, veja o commit a1b2c3d"
aisense task link tsk_7K2 --pr 42           # também --commit, --file, --url
aisense task block tsk_7K2 --reason "preciso da decisão do @arquiteto sobre legacy_id"
aisense task done tsk_7K2 --note "implementado e testado no commit a1b2c3d"
aisense task split tsk_7K2 "parte 1" "parte 2"   # cria subtarefas
```

### Espera

```bash
aisense task watch --mine --timeout 600     # bloqueia até algo meu mudar
```

Permite o padrão "agente de plantão": fica parado sem queimar tokens até chegar trabalho.

## Regras de integridade

### `claim` é atômico
Dois agentes podem tentar pegar o mesmo cartão no mesmo instante. A operação é um
`UPDATE ... WHERE assignee IS NULL AND version = ?` em transação: um ganha, o outro recebe
`already_claimed` com o nome de quem pegou — e a CLI já sugere `aisense task next`.

Sem isso, trabalho duplicado é garantido. **Não é otimização, é requisito.**

### Limite de WIP é aplicado, não decorativo
Mover para uma coluna cheia falha com mensagem acionável:
`"Fazendo está no limite (4/4). Conclua ou devolva um cartão antes de pegar outro."`

### Bloqueio exige motivo
`task block` sem `--reason` é recusado. Um cartão bloqueado sem motivo é lixo no quadro.

### Dependências são verificadas
Mover para `doing` um cartão com dependência aberta gera aviso (não bloqueio) informando qual
cartão falta. Ciclos de dependência são recusados na criação.

### Histórico é imutável
Toda mudança grava uma linha em `activity` com autor (agente ou humano), timestamp e diff.
Nada é apagado — cartão excluído vira `archived`.

## Integração com o barramento

O quadro **não é uma ilha**: toda mudança relevante vira mensagem de sistema para quem interessa.

| Evento | Quem é notificado |
|---|---|
| Cartão atribuído a você | O responsável |
| Comentário no seu cartão | Responsável + criador + participantes do thread |
| Cartão que te bloqueava foi concluído | Responsáveis dos cartões dependentes |
| Seu cartão entrou em Revisão | Quem for definido na automação da coluna |
| Cartão parado em Fazendo > N horas | Responsável, e depois o coordenador |

A notificação respeita o `delivery_mode` do agente (caixa, injeção ou hook) —
mesma máquina de entrega de [07 — Barramento](07-barramento-comunicacao.md).

## Gate de revisão

> Aprovado para o v1 (decisão D6 em [ESTADO.md](ESTADO.md)).

Uma coluna pode exigir aprovação antes de deixar o cartão entrar:

```toml
[[column]]
slug = "done"
requires_approval = true        # alguém precisa aprovar
approver_must_differ = true     # e não pode ser quem fez
requires_commands = ["lint", "test"]   # comandos de aisense.toml que precisam passar
```

```bash
aisense task approve tsk_7K2 --note "testei o fluxo de refresh, está correto"
aisense task reject  tsk_7K2 --reason "o refresh não invalida o token antigo"
```

Regras:

1. **Quem fez não aprova.** Com `approver_must_differ`, o responsável recebe `self_approval`
   e a CLI sugere para quem pedir, olhando os papéis da equipe.
2. **Rejeitar exige motivo.** Sem `--reason`, recusado — igual a bloquear um cartão.
3. Rejeitar devolve o cartão para a coluna anterior e notifica o responsável com o motivo.
4. `requires_commands` roda os comandos de [17 — Comandos do Projeto](17-comandos-do-projeto.md)
   na bancada do responsável. Falhou, o cartão não passa, e a saída fica anexada ao cartão —
   o agente lê o erro sem precisar reproduzir.
5. Aprovação e rejeição entram em `task_activity` com autor e horário. É o registro de quem
   olhou o quê.

É a prática de time de engenharia que mais reduz retrabalho, e aqui sai barata: o quadro já sabe
quem fez, e o barramento já sabe avisar.

## Automações por coluna

Regras declarativas, editáveis na UI, guardadas no quadro:

```toml
[[automation]]
when   = "card_enters"
column = "review"
then   = [
  { assign = "@revisor" },
  { notify = "@revisor", message = "cartão pronto para revisão" },
]

[[automation]]
when   = "card_enters"
column = "doing"
then   = [{ assign = "actor" }]           # quem moveu vira responsável

[[automation]]
when     = "card_stale"
column   = "doing"
after_h  = 4
then     = [{ notify = "@coordenador", message = "cartão parado há 4h" }]

[[automation]]
when   = "card_enters"
column = "done"
then   = [{ unblock_dependents = true }]  # libera quem dependia dele
```

O conjunto de gatilhos do v1 é fechado (`card_created`, `card_enters`, `card_leaves`,
`card_stale`, `checklist_complete`, `comment_added`) e as ações também
(`assign`, `notify`, `move`, `add_label`, `unblock_dependents`, `create_card`).
Automação **não executa comando arbitrário** — isso é [segurança](11-seguranca.md), não limitação.

## Esquema (adendo a [04 — Modelo de Dados](04-modelo-de-dados.md))

```sql
CREATE TABLE boards (
  id         TEXT PRIMARY KEY,
  team_id    TEXT NOT NULL UNIQUE REFERENCES teams(id) ON DELETE CASCADE,
  automations TEXT NOT NULL DEFAULT '[]',   -- JSON
  created_at INTEGER NOT NULL
);

CREATE TABLE columns (
  id        TEXT PRIMARY KEY,
  board_id  TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  slug      TEXT NOT NULL,
  name      TEXT NOT NULL,
  kind      TEXT NOT NULL,                  -- intake|ready|active|blocked|review|terminal
  wip_limit INTEGER,                        -- NULL = sem limite
  position  INTEGER NOT NULL,
  UNIQUE (board_id, slug)
);

-- A tabela `tasks` de 04-modelo-de-dados.md passa a referenciar column_id e ganha:
ALTER TABLE tasks ADD COLUMN column_id  TEXT REFERENCES columns(id);
ALTER TABLE tasks ADD COLUMN priority   TEXT NOT NULL DEFAULT 'normal'; -- low|normal|high|urgent
ALTER TABLE tasks ADD COLUMN labels     TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN checklist  TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN links      TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN block_reason TEXT;
ALTER TABLE tasks ADD COLUMN version    INTEGER NOT NULL DEFAULT 1;     -- trava otimista do claim
ALTER TABLE tasks ADD COLUMN archived_at INTEGER;
ALTER TABLE tasks ADD COLUMN approved_by  TEXT REFERENCES agents(id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN approved_at  INTEGER;

ALTER TABLE columns ADD COLUMN requires_approval    INTEGER NOT NULL DEFAULT 0;
ALTER TABLE columns ADD COLUMN approver_must_differ INTEGER NOT NULL DEFAULT 1;
ALTER TABLE columns ADD COLUMN requires_commands    TEXT NOT NULL DEFAULT '[]';

CREATE TABLE task_dependencies (
  task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  depends_on TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, depends_on),
  CHECK (task_id <> depends_on)
);

CREATE TABLE task_comments (
  id         TEXT PRIMARY KEY,
  task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author     TEXT REFERENCES agents(id) ON DELETE SET NULL,  -- NULL = humano
  body       TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE task_activity (
  id         TEXT PRIMARY KEY,
  task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author     TEXT REFERENCES agents(id) ON DELETE SET NULL,
  action     TEXT NOT NULL,       -- created|moved|assigned|commented|blocked|done|...
  detail     TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_activity_task ON task_activity(task_id, created_at);
```

## Interface (T8 revisada)

```
┌ Quadro — Squad Produto ─── [+ Cartão] [filtros ▾] [⚙ colunas] [automações] ───┐
│                                                                                │
│ A FAZER (4)      FAZENDO (2/4)    BLOQUEADA (1)   REVISÃO (1)   FEITA (12)     │
│ ┌────────────┐   ┌────────────┐   ┌───────────┐   ┌──────────┐  ┌───────────┐ │
│ │▏OAuth em   │   │▏Middleware │   │▏Remover   │   │▏Endpoint │  │▏Migração  │ │
│ │ /users     │   │ de refresh │   │ legacy_id │   │ /auth    │  │ de schema │ │
│ │ ▲alta  ⛓2  │   │ ✓2/4  💬3  │   │ ⚠ aguarda │   │ @backend │  │ ✓ há 1h   │ │
│ │ sem dono   │   │ @backend   │   │ decisão   │   │ →@revisor│  │           │ │
│ └────────────┘   └────────────┘   └───────────┘   └──────────┘  └───────────┘ │
│ ┌────────────┐   ┌────────────┐                                                │
│ │▏Cliente TS │   │▏Testes de  │                                                │
│ │ ⛓ bloqueado│   │ integração │                                                │
│ └────────────┘   └────────────┘                                                │
└────────────────────────────────────────────────────────────────────────────────┘
```

- Borda esquerda do cartão na **cor do responsável** — o mesmo código visual dos terminais.
- Ícones: `▲` prioridade, `⛓` dependências, `✓n/m` checklist, `💬n` comentários, `⚠` bloqueio.
- Arrastar entre colunas dispara as mesmas regras da API (inclusive WIP e automações).
- Cartão aberto: corpo em Markdown, checklist, thread de comentários (com avatar do agente),
  dependências navegáveis, links para PR/commit e o histórico completo.
- Cartões mudando aparecem com um realce de 400 ms — dá para ver os agentes trabalhando ao vivo.
- Um contador no topo: "3 cartões mudaram desde que você saiu" com link para o diff do quadro.

## Erros que a CLI precisa devolver bem

A qualidade da mensagem de erro define se a IA se recupera sozinha ou trava:

| Código | Mensagem devolvida |
|---|---|
| `already_claimed` | `tsk_7K2 já foi pego por @frontend há 30s. Use 'aisense task next' para o próximo.` |
| `wip_exceeded` | `Fazendo está no limite (4/4). Conclua ou devolva um cartão antes de pegar outro.` |
| `blocked_by_open` | `tsk_7K3 depende de tsk_7K2 (aberto). Siga assim mesmo com --force ou trabalhe no tsk_7K2.` |
| `reason_required` | `Bloquear exige --reason. Diga o que falta e quem pode destravar.` |
| `unknown_column` | `Coluna 'em-progresso' não existe. Colunas: backlog, todo, doing, blocked, review, done.` |
| `dependency_cycle` | `Isso criaria um ciclo: tsk_A → tsk_B → tsk_A.` |
| `self_approval` | `Você fez este cartão, então não pode aprová-lo. Peça a @revisor ou @arquiteto.` |
| `gate_failed` | `O comando 'test' falhou (exit 1). A saída está anexada ao cartão.` |
