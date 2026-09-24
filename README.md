<h1 align="center">AISENSE</h1>
<p align="center"><strong>Crie times de IA de maneira fácil e gerenciável.</strong></p>

---

AISENSE é um aplicativo de desktop para **montar, operar e coordenar equipes de agentes de IA**.
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

## Instalação

Baixe o instalador do seu sistema na página de
[Releases](https://github.com/jjoaquim0/AISENSE/releases/latest):

| Sistema | Arquivo | Como instalar |
|---|---|---|
| macOS 11+ (Apple Silicon e Intel) | `AISENSE_<versão>_universal.dmg` | Abra o `.dmg` e arraste o AISENSE para Aplicativos |
| Windows 10/11 | `AISENSE_<versão>_x64-setup.exe` (ou o `.msi`) | Execute o instalador; não precisa de administrador no `-setup.exe` |
| Ubuntu, Debian e derivados | `AISENSE_<versão>_amd64.deb` | `sudo apt install ./AISENSE_<versão>_amd64.deb` |
| Qualquer Linux x86_64 | `AISENSE_<versão>_amd64.AppImage` | `chmod +x AISENSE_*.AppImage` e execute |

O AISENSE hospeda as CLIs de IA; ele não traz nenhuma. Instale pelo menos uma das que você quer usar
(sem nenhuma, dá para começar com agentes de terminal puro):

| IA | Instalação |
|---|---|
| Claude Code | `npm i -g @anthropic-ai/claude-code` |
| Codex CLI | `npm i -g @openai/codex` |
| OpenCode | `npm i -g opencode-ai` |
| Gemini CLI | `npm i -g @google/gemini-cli` |

As versões novas chegam sozinhas: o app confere se há atualização ao abrir e pergunta antes de
instalar. Dá para desligar em **Configurações → Avançado**.

## Primeiros passos

1. **Abra o AISENSE.** O primeiro passo mostra quais IAs ele encontrou no seu computador. Se alguma
   que você instalou não aparece, veja [Solução de problemas](docs/guia/problemas.md#runtimes).
2. **Escolha o tema** (dá para trocar depois com `⌘⇧D`/`Ctrl+Shift+D`).
3. **Crie a primeira equipe:** um nome, a pasta do projeto em que ela vai trabalhar e um modelo —
   *Dupla Dev* (`@dev` e `@revisor`), *Squad completo* (`@arquiteto`, `@backend`, `@frontend`,
   `@revisor`), *Pesquisa*, *Operação* ou *Vazio*. A equipe é criada e iniciada: os terminais sobem
   na **Sala da Equipe**.
4. **Dê a missão.** Clique no terminal de um agente e fale com a IA como faria num terminal comum.
   Cada agente já nasceu sabendo quem é, quem são os colegas e como falar com eles.
5. **Veja a equipe conversando.** Os agentes usam a CLI `aisense` (ou o servidor MCP) entre si —
   `aisense send @revisor "pronto para revisar"`, `aisense ask @arquiteto "qual contrato?"`. Tudo
   aparece na vista **Mensagens**; o trabalho, na vista **Quadro**; quem fala com quem, na vista
   **Fluxo**. Você também escreve na linha do tempo como `@voce`.

Para ir além:

- [Criar um adaptador para outra IA](docs/guia/adaptadores.md)
- [Criar e atribuir skills](docs/guia/skills.md)
- [Solução de problemas](docs/guia/problemas.md)
- `⌘K` / `Ctrl+K` abre a paleta com todos os comandos do app.

## Desenvolvimento

O estado do projeto e o plano por fases estão em [`docs/ESTADO.md`](docs/ESTADO.md) e
[`docs/INDEX.md`](docs/INDEX.md). Para compilar:

```bash
pnpm install
pnpm app          # app em modo de desenvolvimento (Tauri)
pnpm app:build    # instaladores do seu sistema em target/release/bundle/
pnpm test         # testes do front e do core
```

No Linux, o app precisa de `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev
librsvg2-dev libsoup-3.0-dev patchelf`.

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
| 7 | Coordenação: agente coordenador, vista Fluxo, canais | [FASE-07](docs/fases/FASE-07-coordenacao.md) |
| 8 | Acabamento: temas, a11y, performance | [FASE-08](docs/fases/FASE-08-acabamento.md) |
| 9 | Distribuição: instaladores, auto-update | [FASE-09](docs/fases/FASE-09-distribuicao.md) |

## Para agentes de IA que forem desenvolver este projeto

Leia **[`AGENTS.md`](AGENTS.md)** antes de qualquer coisa. Ele define o ritual de início de sessão,
onde encontrar o estado atual do projeto e como registrar o que você fez.

## Licença

MIT — veja [LICENSE](LICENSE).
