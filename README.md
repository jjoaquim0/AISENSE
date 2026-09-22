<h1 align="center">AISENSE</h1>
<p align="center"><strong>Crie times de IA de maneira fácil e gerenciável.</strong></p>

---

AISENSE é um aplicativo de desktop para **montar, operar e orquestrar equipes de agentes de IA**.
Cada equipe tem uma missão. Dentro da equipe você cria agentes, e cada agente é um **terminal real**
rodando a IA que você escolher — Claude Code, Codex, OpenCode, Gemini CLI ou um shell puro onde você
chama o que quiser.

O diferencial: **os terminais conversam entre si**. Um agente pode mandar mensagem, fazer pergunta e
esperar resposta de outro agente, delegar tarefas e acompanhar o quadro da equipe — tudo através de um
barramento de mensagens nativo do AISENSE (CLI `aisense` + servidor MCP).

E cada agente sobe **já sabendo o que fazer**: você configura skills por agente, e no boot do terminal
o AISENSE materializa essas skills no diretório de trabalho e injeta o prompt de bootstrap.

```
┌─ Equipes ─┬─ Squad Produto ──────────────────────────────────────────────┐
│  ◍ Produto│  @arquiteto      @backend       @frontend      @revisor      │
│  ◍ Infra  │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  │
│  ◍ Pesq.  │  │ claude    │  │ codex     │  │ opencode  │  │ claude    │  │
│  +        │  │ ● pensando│  │ ● ocioso  │  │ ● ocioso  │  │ ● aguarda │  │
│           │  └───────────┘  └───────────┘  └───────────┘  └───────────┘  │
└───────────┴──────────────────────────────────────────────────────────────┘
```

## Status

🚧 **Fase de projeto.** O código ainda não foi escrito — a documentação completa de arquitetura,
design e plano de execução está pronta em [`docs/`](docs/).

**Comece por [`docs/INDEX.md`](docs/INDEX.md).**

## Stack

| Camada | Tecnologia | Porquê |
|---|---|---|
| Shell do app | **Tauri 2** | Webview nativa: ~40 MB RAM de base contra ~300 MB do Electron |
| Core | **Rust** | PTY, barramento de mensagens, IPC e persistência sem GC e sem travar a UI |
| Interface | **React 19 + TypeScript + Vite** | Ecossistema maduro para UI complexa com muitos painéis |
| Estilo | **Tailwind CSS 4 + Radix Primitives** | Tokens em OKLCH, tema claro/escuro, acessibilidade de graça |
| Terminal | **xterm.js + renderer WebGL** | Único terminal web que aguenta múltiplas instâncias a 60fps |
| Dados | **SQLite (SQLx)** | Local-first, sem servidor, transacional |

Justificativa completa e alternativas descartadas: [`docs/03-stack.md`](docs/03-stack.md) e [`docs/adr/`](docs/adr/).

## Roadmap

| Fase | Entrega | Doc |
|---|---|---|
| 0 | Fundação: monorepo, Tauri, CI, tokens de design | [FASE-00](docs/fases/FASE-00-fundacao.md) |
| 1 | Terminal Core: PTY em Rust + xterm.js | [FASE-01](docs/fases/FASE-01-terminal-core.md) |
| 2 | Equipes e agentes: CRUD, SQLite, adaptadores | [FASE-02](docs/fases/FASE-02-equipes-agentes.md) |
| 3 | Sala da Equipe: grid, foco, ciclo de vida | [FASE-03](docs/fases/FASE-03-sala-da-equipe.md) |
| 4 | Sistema de Skills | [FASE-04](docs/fases/FASE-04-skills.md) |
| 5 | Barramento: agentes conversando entre si | [FASE-05](docs/fases/FASE-05-barramento.md) |
| 6 | Quadro Kanban compartilhado com os agentes | [FASE-06](docs/fases/FASE-06-quadro-kanban.md) |
| 7 | Orquestração: maestro, canvas, canais | [FASE-07](docs/fases/FASE-07-orquestracao.md) |
| 8 | Acabamento: temas, a11y, performance | [FASE-08](docs/fases/FASE-08-acabamento.md) |
| 9 | Distribuição: instaladores, auto-update | [FASE-09](docs/fases/FASE-09-distribuicao.md) |

## Para agentes de IA que forem desenvolver este projeto

Leia **[`AGENTS.md`](AGENTS.md)** antes de qualquer coisa. Ele define o ritual de início de sessão,
onde encontrar o estado atual do projeto e como registrar o que você fez.

## Licença

MIT — veja [LICENSE](LICENSE).
