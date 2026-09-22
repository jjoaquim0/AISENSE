# 02 — Arquitetura

## Visão em camadas

```
┌──────────────────────────────────────────────────────────────────────────┐
│  APRESENTAÇÃO — React 19 + TypeScript (webview)                          │
│  Sala da Equipe · Painéis xterm.js · Inspetor · Linha do tempo · ⌘K      │
└───────────────────────────────▲──────────────────────────────────────────┘
        comandos (invoke)       │      eventos (emit, ~60fps, coalescidos)
┌───────────────────────────────▼──────────────────────────────────────────┐
│  aisense-app — camada fina Tauri: comandos, eventos, estado global       │
│  Não contém regra de negócio. Só traduz UI ⇄ core.                       │
└───────────────────────────────▲──────────────────────────────────────────┘
┌───────────────────────────────▼──────────────────────────────────────────┐
│  aisense-core — DOMÍNIO                                                  │
│  Team · Agent · Skill · Message · Task · Supervisor · políticas          │
│  Puro: sem Tauri, sem UI, sem I/O direto. 100% testável com cargo test.  │
└──────┬─────────────────┬────────────────┬────────────────┬───────────────┘
       │                 │                │                │
┌──────▼──────┐  ┌───────▼──────┐  ┌──────▼──────┐  ┌──────▼─────────────┐
│ aisense-pty │  │aisense-store │  │ aisense-ipc │  │ adapters (TOML)    │
│ PTY, ring   │  │ SQLite/SQLx  │  │ UDS / pipe  │  │ claude, codex,     │
│ buffer,     │  │ migrações    │  │ NDJSON      │  │ opencode, shell... │
│ estado      │  │ repositórios │  │ auth token  │  │                    │
└──────┬──────┘  └──────────────┘  └──────▲──────┘  └────────────────────┘
       │ spawn                            │ conecta
┌──────▼───────────────────────────────┐  │
│ PROCESSOS DOS AGENTES (PTY)          │  │
│  claude · codex · opencode · bash    │──┘  via `aisense` CLI ou `aisense-mcp`
└──────────────────────────────────────┘
```

**Regra de ouro:** `core` não conhece `app`. `app` é substituível (um dia pode ser um daemon headless).

## Os sete crates

| Crate | Responsabilidade | Não pode |
|---|---|---|
| `aisense-core` | Modelos de domínio, roteamento de mensagens, resolução de skills, máquina de estados do agente, políticas de entrega | Importar Tauri, tocar em disco diretamente, conhecer SQL |
| `aisense-pty` | Spawn/kill/resize de PTY, bombeamento de saída, ring buffer, detecção de ocioso | Conhecer equipes ou mensagens |
| `aisense-store` | Esquema SQLite, migrações, repositórios (traits definidas no core) | Conter regra de negócio |
| `aisense-ipc` | Servidor de socket local, autenticação por token, protocolo NDJSON | Conter regra de negócio (delega ao core) |
| `aisense-cli` | Binário `aisense` usado *dentro* dos terminais | Falar com SQLite direto — só via socket |
| `aisense-mcp` | Servidor MCP stdio expondo o barramento como ferramentas | Idem |
| `aisense-app` | Comandos e eventos Tauri, bootstrap, bandeja, janelas | Conter regra de negócio |

Por que `aisense-cli` e `aisense-mcp` são binários separados e não o app: eles rodam **dentro** do
terminal do agente, como processos filhos efêmeros. Precisam iniciar em <20 ms e pesar poucos MB.
São distribuídos como *sidecars* do Tauri e injetados no `PATH` de cada PTY.

## Fluxo 1 — Subir um agente

```
UI: "Iniciar @backend"
 └─ invoke("agent_start", { agentId })
     └─ core::AgentSupervisor::start(agent)
         1. resolve o adaptador (adapters/codex.toml)
         2. resolve as skills atribuídas          → docs/06
         3. materializa skills em <cwd>/.aisense/skills/ (+ .claude/skills/ se aplicável)
         4. gera .aisense/BOOT.md e .aisense/agent.json
         5. monta env: AISENSE_SOCKET, AISENSE_AGENT_ID, AISENSE_TOKEN,
                       AISENSE_TEAM_ID, PATH += <dir dos sidecars>
         6. pty::spawn(cmd, args, cwd, env)
         7. registra o agente no bus (endereço @backend fica ativo)
         8. injeta o bootstrap conforme a capacidade do adaptador:
            • flag de system prompt  (ex.: --append-system-prompt)  ← preferido
            • MCP                    (aisense-mcp já no mcp config)
            • stdin                  (digita uma linha "leia .aisense/BOOT.md")
 └─ evento agent:state { id, state: "starting" → "idle" }
```

## Fluxo 2 — Saída do terminal chegando na tela

O caminho crítico de performance. Um agente verboso emite dezenas de milhares de bytes por segundo;
nove agentes fazem isso em paralelo.

```
processo → PTY master → tarefa de leitura (Rust, 1 por agente)
    ├─→ RingBuffer (últimas N linhas, default 10.000) — fonte da verdade para reidratar
    ├─→ arquivo de log append-only (~/.aisense/logs/<agent>.log) — histórico completo
    ├─→ detector de estado (ocioso/ocupado/aguardando) — ver docs/05
    └─→ canal broadcast → coalescedor (janela de 16 ms)
            └─→ SE o painel está visível: emit "pty:data" { agentId, chunk }
                SENÃO: não emite nada. O buffer do Rust acumula; ao focar, a UI
                       pede pty_snapshot(agentId) e reidrata o xterm de uma vez.
```

Três decisões que fazem isso ser rápido:
1. **Coalescência de 16 ms** — no máximo 60 eventos/s por agente, em vez de milhares.
2. **Painel invisível não renderiza.** xterm.js só existe para o que está na tela.
3. **Ring buffer em Rust**, não no JS. A memória do histórico não fica no heap da webview.

## Fluxo 3 — Um agente falando com outro

```
@backend (dentro do PTY) executa:  aisense ask @frontend "o contrato do /users mudou?"
   │
   ├─ aisense-cli lê AISENSE_SOCKET e AISENSE_TOKEN do ambiente
   ├─ conecta no UDS e envia frame NDJSON { op:"ask", to:"@frontend", body:"...", timeout:300 }
   │
   └─ aisense-ipc autentica o token → resolve para agent_id
        └─ core::Bus::route(msg)
             ├─ persiste em messages (SQLite)
             ├─ emite evento p/ UI (linha do tempo atualiza na hora)
             └─ entrega em @frontend conforme a política do agente:
                  • pull      → fica na caixa de entrada; badge acende na UI
                  • push      → enfileira; quando o detector disser "ocioso",
                                escreve no stdin do PTY do @frontend
                  • hook      → o próprio agente checa ao fim de cada turno
        └─ @frontend responde com  aisense reply <msg_id> "..."
             └─ correlaciona pelo reply_to, destrava o ask do @backend,
                que imprime a resposta no stdout e encerra com exit 0
```

Detalhe importante: o `ask` **bloqueia o processo `aisense`**, não o terminal inteiro. Para a IA
chamadora, é uma chamada de ferramenta síncrona comum — igual a rodar um teste que demora.

## Máquina de estados do agente

```
  ┌─────────┐  start   ┌──────────┐  pty pronto  ┌──────┐
  │ stopped ├─────────►│ starting ├─────────────►│ idle │◄──────┐
  └─────────┘          └────┬─────┘              └──┬───┘       │
       ▲                    │ falha                 │ entrada   │ saída
       │                    ▼                       ▼           │ cessa
       │ kill/exit     ┌────────┐              ┌──────┐         │
       └───────────────┤ failed │              │ busy ├─────────┘
                       └────────┘              └──┬───┘
                                                  │ pede confirmação
                                          ┌───────▼────────┐
                                          │ awaiting_input │
                                          └────────────────┘
```

- `idle` → seguro injetar mensagem.
- `busy` → enfileira.
- `awaiting_input` → **nunca** injeta; destaca na UI para o humano decidir.
- `failed` → política de reinício por agente (`never` | `on-crash` | `always`, com backoff).

Como o estado é detectado (heurística por adaptador, configurável em TOML): silêncio na saída por
`quiet_ms` **e** última linha casando com `idle_regex`. Detalhes e limitações em
[ADR 0006](adr/0006-entrega-de-mensagens.md).

## Concorrência no Rust

- Runtime **Tokio** multi-thread.
- Uma task de leitura por PTY (I/O bloqueante de PTY vai em `spawn_blocking`).
- Estado compartilhado em `Arc<RwLock<...>>` com **escopo curto** — nunca segure lock atravessando `await`.
- Comunicação entre subsistemas por `tokio::sync::broadcast` (fan-out para UI) e `mpsc` (comandos).
- Cada agente tem um `CancellationToken` — parar agente cancela leitura, detector e entregas pendentes.

## Persistência e layout em disco

```
~/.aisense/                      (ou %APPDATA%\AISENSE no Windows)
├── aisense.db                   SQLite: equipes, agentes, mensagens, tarefas, skills
├── config.toml                  preferências globais
├── adapters/                    TOMLs de runtime (embutidos + do usuário)
├── skills/                      biblioteca de skills do usuário
├── logs/<agent_id>.log          transcrição completa de cada terminal
└── run/aisense.sock             socket do barramento (named pipe no Windows)
```

Já o **diretório de trabalho da equipe** (escolhido pelo usuário, ex.: seu repositório) recebe:

```
<workdir>/.aisense/
├── agent.json                   identidade do agente (para a IA se reconhecer)
├── BOOT.md                      prompt de bootstrap gerado
├── skills/<nome>/SKILL.md       skills materializadas
└── inbox/                       (opcional) mensagens como arquivos, para runtimes limitados
```

`.aisense/` no diretório do usuário é descartável: pode ser regenerado a qualquer boot e deve entrar
no `.gitignore` sugerido pelo app.

## Onde estender

| Quero adicionar... | Mexo em | Preciso recompilar? |
|---|---|---|
| Um runtime de IA novo | `adapters/*.toml` | **Não** |
| Uma skill | `~/.aisense/skills/` ou a biblioteca na UI | **Não** |
| Uma política de entrega | `aisense-core::delivery` | Sim |
| Uma visualização da equipe | `apps/desktop/src/features/team-room/views/` | Sim (só front) |
| Uma ferramenta no barramento | `aisense-core::bus::ops` + `aisense-cli` + `aisense-mcp` | Sim |
