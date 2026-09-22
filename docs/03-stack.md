# 03 — Stack Tecnológica

> Resumo das escolhas com as alternativas que foram descartadas e o motivo.
> Mudanças aqui exigem ADR.

## O que o produto exige da tecnologia

Antes de escolher, os requisitos não-negociáveis:

| # | Requisito | Consequência técnica |
|---|---|---|
| RQ1 | Rodar 6–12 PTYs reais simultâneos, alguns por horas | Precisa de I/O assíncrono barato e memória previsível |
| RQ2 | Terminal fiel (cores, TUI, redimensionamento) | Precisa de PTY de verdade, não `child_process` com pipes |
| RQ3 | UI densa com muitos painéis e transições suaves | Precisa de um ecossistema de UI maduro |
| RQ4 | Um binário instalável nos 3 SOs | Precisa de cross-compilation e instaladores |
| RQ5 | Processos externos precisam falar com o app | Precisa de IPC local (socket/pipe), não só memória |
| RQ6 | App leve — ele é infraestrutura, não o trabalho em si | Orçamento: <150 MB RAM com 6 agentes ociosos |

## Decisões

### Shell do aplicativo — **Tauri 2**

| Opção | Prós | Contras | Veredito |
|---|---|---|---|
| **Tauri 2** | Webview do SO (~40 MB base), backend Rust nativo, bundle ~10 MB, sidecars nativos, IPC tipada | Webview varia por SO (WebKitGTK no Linux), ecossistema menor | ✅ **Escolhido** |
| Electron | Chromium igual em todo lugar, ecossistema enorme, `node-pty` maduro | ~300 MB base, bundle ~150 MB, gestão de PTY em Node custa CPU | ❌ Fere RQ6 |
| Web + daemon local | Acessível do navegador | Instalação em duas partes, segurança de origem, UX de app pior | ❌ Fora do v1 (ver D4) |
| Nativo (SwiftUI/WinUI/GTK) | Melhor performance e integração | 3 UIs para manter; inviável | ❌ |

→ [ADR 0001](adr/0001-tauri-em-vez-de-electron.md)

### Linguagem do core — **Rust**

Motivo direto: o core é **99% I/O concorrente e gestão de processos**. É exatamente onde Rust +
Tokio brilham e onde uma linguagem com GC sofre (pausas atrapalham o pump de PTY, e é aí que o
terminal "engasga"). Além disso, Tauri já exige Rust — usar outra linguagem no core significaria
FFI por nada.

Bibliotecas principais:

| Crate | Para quê |
|---|---|
| `tokio` | Runtime assíncrono |
| `portable-pty` | PTY multiplataforma (macOS/Linux via `openpty`, Windows via ConPTY) |
| `sqlx` (SQLite, offline mode) | Persistência com queries verificadas em tempo de compilação |
| `serde` / `serde_json` | Serialização do protocolo e dos modelos |
| `ts-rs` | Gera tipos TypeScript a partir dos structs Rust — uma única fonte da verdade |
| `thiserror` | Erros de domínio tipados |
| `tracing` + `tracing-subscriber` | Logs estruturados |
| `toml` | Adaptadores e configuração |
| `gray_matter` + `pulldown-cmark` | Parse de skills (frontmatter + Markdown) |
| `keyring` | Segredos no keychain do SO |
| `notify` | Hot-reload de skills e adaptadores em disco |
| `interprocess` | Socket Unix + named pipe do Windows sob a mesma API |
| `ulid` | IDs ordenáveis por tempo (melhor que UUIDv4 para índice de mensagens) |

→ [ADR 0002](adr/0002-core-em-rust.md)

### Interface — **React 19 + TypeScript + Vite**

| Opção | Veredito |
|---|---|
| **React 19 + TS** | ✅ Ecossistema insuperável para UI complexa; xterm.js, dnd-kit, Radix, virtualização, tudo pronto |
| Svelte 5 | Mais enxuto e rápido, mas ecossistema menor para componentes densos |
| SolidJS | Performance excelente, comunidade pequena demais para o tamanho desta UI |
| Leptos/Dioxus (Rust no front) | Stack única seria elegante, mas xterm.js é JS de qualquer jeito e a produtividade cai muito |

Bibliotecas do front:

| Pacote | Para quê |
|---|---|
| `@xterm/xterm` + `addon-webgl` + `addon-fit` + `addon-search` | Emulação de terminal |
| `tailwindcss@4` | Estilos com tokens em CSS nativo (`@theme`) |
| `@radix-ui/react-*` | Primitivas acessíveis (dialog, dropdown, tooltip, tabs) |
| `zustand` | Estado global leve — sem boilerplate de store |
| `@tanstack/react-query` | Cache e invalidação do estado vindo do Rust |
| `dnd-kit` | Arrastar painéis no grid e cards no quadro |
| `motion` (ex-framer-motion) | Transições; usado com parcimônia |
| `cmdk` | Paleta de comandos ⌘K |
| `@xyflow/react` | Vista Canvas/Fluxo da equipe (agentes como nós) |
| `lucide-react` | Ícones |

### Terminal — **xterm.js com renderer WebGL**

Não há alternativa séria em web. Pontos de atenção documentados na Fase 1:
uma instância por painel visível, `WebglAddon` obrigatório (o renderer DOM não aguenta),
`FitAddon` em `ResizeObserver` com debounce, e `dispose()` religioso ao desmontar.

### Dados — **SQLite via SQLx**

Local-first, transacional, zero configuração, ótimo para a linha do tempo de mensagens
(que é essencialmente um log append-only consultado por intervalo).
WAL ligado. Migrações versionadas em `crates/aisense-store/migrations/`.
→ [ADR 0003](adr/0003-sqlite-local-first.md)

### IPC com os processos dos agentes — **NDJSON sobre socket local**

Unix domain socket no macOS/Linux, named pipe no Windows, abstraídos por `interprocess`.
Uma linha JSON por frame. Autenticação por token de curta duração, único por agente, passado via env.
Descartados: HTTP local (porta exposta a qualquer processo, CORS, overhead), gRPC (peso desnecessário),
arquivos em disco (sem push, sem request/reply decente).
→ [ADR 0004](adr/0004-protocolo-do-barramento.md)

### Tooling

| Ferramenta | Uso |
|---|---|
| `pnpm` + workspaces | Gerenciador de pacotes do front |
| `cargo` workspace | Crates do Rust |
| `biome` | Lint + format do TS (substitui ESLint+Prettier, muito mais rápido) |
| `rustfmt` + `clippy -D warnings` | Lint do Rust |
| `vitest` + `@testing-library/react` | Testes de front |
| `cargo test` + `insta` (snapshots) | Testes de Rust |
| `playwright` | E2E do app empacotado |
| GitHub Actions | CI nos 3 SOs |

## Orçamento de performance (metas verificáveis na Fase 8)

| Métrica | Meta |
|---|---|
| Cold start até a Sala da Equipe utilizável | < 1,2 s |
| RAM com 6 agentes ociosos | < 150 MB |
| RAM com 12 agentes ativos | < 400 MB |
| CPU ociosa (agentes parados) | < 1% |
| Latência de tecla → eco no terminal focado | < 16 ms |
| Latência de `aisense send` → mensagem na linha do tempo | < 50 ms |
| Troca de painel focado (reidratação do buffer) | < 100 ms |
| Tamanho do instalador | < 25 MB |
