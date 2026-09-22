# 16 — Bancadas

> **O problema que isto resolve:** dois agentes editando o mesmo checkout se atropelam. Um salva
> por cima do outro, o `git status` vira uma sopa e ninguém sabe qual mudança é de quem.
> Sem isolamento, "equipe de agentes" na prática só funciona com um agente escrevendo por vez.

Uma **bancada** é um `git worktree` com branch próprio, por agente.
Aprovado para o v1 (decisão D6 em [ESTADO.md](ESTADO.md)).

## Os dois modos, por equipe

| Modo | Quando usar |
|---|---|
| `shared` (padrão) | Um agente escrevendo por vez, ou agentes que só leem. Simples, sem surpresa |
| `per-agent` | Vários agentes implementando em paralelo. Cada um na sua bancada |

O modo é da equipe (`teams.workspace_mode`), com exceção por agente
(`agents.workbench` = `inherit` | `own` | `shared`). Um `@revisor` que só lê pode ficar no
checkout compartilhado mesmo numa equipe `per-agent`.

## Como funciona

```
~/.aisense/benches/<team_id>/<handle>/       ← worktree, FORA do repositório
```

Fica fora do repositório de propósito: worktree dentro do próprio repo polui `git status`,
confunde ferramentas e acaba commitado por acidente.

No start do agente:

```
1. o diretório da equipe é um repositório git?
     não  → cai para `shared` com aviso visível na UI (não é erro, é limitação)
2. a bancada já existe e é válida (`git worktree list` a reconhece)?
     sim  → reutiliza
3. cria:  git worktree add <dir> -b aisense/<handle> <branch_base>
     branch já existe → reaproveita em vez de falhar
4. AISENSE_WORKDIR e o cwd do PTY apontam para a bancada
5. materializa skills e notas na bancada, como em qualquer diretório de trabalho
```

`branch_base` é configurável por equipe; o padrão é a branch atual no momento da criação.

## API dos agentes

```bash
aisense bench                      # onde estou, em que branch, quantas mudanças pendentes
aisense bench sync                 # traz a branch base para dentro da minha bancada (merge)
aisense bench diff                 # diff da bancada contra a base
aisense bench publish              # push da branch; imprime a URL do PR quando há remoto
aisense bench list                 # todas as bancadas da equipe e o estado de cada uma
```

`aisense bench sync` usa **merge, nunca rebase**: a bancada é um branch compartilhado com o
AISENSE e reescrever histórico dela quebra a referência do worktree.

## Integração com o quadro

Cada cartão registra em que branch está sendo feito. A coluna `Revisão` mostra o link do PR quando
existe, e o [gate de revisão](13-quadro-kanban.md#gate-de-revisão) pode exigir que a bancada esteja
publicada antes de fechar o cartão.

## Ciclo de vida e limpeza

| Evento | O que acontece |
|---|---|
| Agente parado | A bancada **fica**. Trabalho em andamento não se joga fora |
| Agente excluído | Pergunta o que fazer; com mudanças não commitadas, **recusa** e explica |
| Equipe excluída | Lista as bancadas com pendências antes de confirmar |
| `Limpar bancadas` (UI) | Remove só as que estão limpas e já mergeadas na base |

Remoção sempre por `git worktree remove` — nunca `rm -rf`, que deixa o repositório com worktree
fantasma registrado.

## Limitações conhecidas (documentar na UI, não esconder)

| Situação | Comportamento |
|---|---|
| Diretório não é git | Cai para `shared` com aviso |
| Submódulos | O worktree não os inicializa sozinho; o app roda `git submodule update --init` se houver `.gitmodules` |
| Git LFS | Cada bancada baixa os objetos de novo. Aviso de espaço em disco ao ativar |
| Arquivos ignorados (`.env`, `node_modules`) | **Não** são copiados pelo git. O app oferece copiar um conjunto declarado em `aisense.toml` ([17](17-comandos-do-projeto.md)) — sem isso o agente sobe numa bancada que não builda |
| Git < 2.5 | Sem suporte a worktree; cai para `shared` com aviso |

O item dos arquivos ignorados é o que mais morde na prática: a bancada tem o código, mas não tem
`node_modules` nem `.env`, e o agente perde a primeira meia hora descobrindo isso sozinho.

## Esquema (adendo a [04 — Modelo de Dados](04-modelo-de-dados.md))

```sql
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
```
