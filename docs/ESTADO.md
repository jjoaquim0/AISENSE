# ESTADO DO PROJETO

> **Este é o arquivo mais importante do repositório para quem chega.**
> Toda sessão de desenvolvimento começa lendo isto e termina atualizando isto.
> Se estiver desatualizado, o próximo agente se perde. Mantenha-o honesto.

**Última atualização:** 2026-09-22
**Fase atual:** `FASE 02 — Equipes e Agentes` (em andamento; 00 e 01 com pendências só visuais)
**Branch de desenvolvimento:** `claude/gifted-cray-29owpm`

---

## Situação em uma frase

A documentação completa de produto, arquitetura, design e plano de execução está escrita.
**Nenhuma linha de código de aplicação existe ainda.** O próximo passo é a Fase 0 (scaffold do monorepo).

## Progresso por fase

| Fase | Status | Tarefas |
|---|---|---|
| 00 — Fundação | 🟨 Em andamento | 6 feitas, 3 parciais de 9 |
| 01 — Terminal Core | 🟨 Em andamento | 4 feitas, 4 parciais de 8 |
| 02 — Equipes e Agentes | 🟨 Em andamento | 1 feita de 11 |
| 03 — Sala da Equipe | ⬜ Não iniciada | 0/9 |
| 04 — Sistema de Skills | ⬜ Não iniciada | 0/9 |
| 05 — Barramento | ⬜ Não iniciada | 0/13 |
| 06 — Quadro Kanban | ⬜ Não iniciada | 0/11 |
| 07 — Coordenação | ⬜ Não iniciada | 0/6 |
| 08 — Acabamento | ⬜ Não iniciada | 0/9 |
| 09 — Distribuição | ⬜ Não iniciada | 0/7 |

Legenda: ⬜ não iniciada · 🟨 em andamento · ✅ concluída · 🟥 bloqueada

## Em andamento agora

**Fase 02 — Equipes e Agentes** (iniciada em 2026-09-22 a pedido do usuário). Exceção consciente à
regra R1: o que resta das Fases 00 e 01 é **só verificação visual/CI** (F00-02, F00-04, F00-09,
F01-05..08), nada de código faltando, e nada da Fase 02 depende disso. As pendências continuam
abertas e devem ser fechadas por quem tiver uma máquina com tela.

| Tarefa | Situação |
|---|---|
| F02-01 Modelos de domínio | ✅ `Team`, `Agent`, `Handle`, políticas, estados, portas de repositório e `InMemoryStore` |


**Fase 00 — Fundação.** O esqueleto está de pé e verificado no que dava para verificar:

| Verificado aqui | Resultado |
|---|---|
| `cargo test --workspace --exclude aisense-app` | ✅ 63 testes (14 core + 49 pty) |
| `pnpm --filter @aisense/desktop test --run` | ✅ 99 testes (67 de contraste/cor + 5 de layout + 6 de decodificação, entre outros) |
| `pnpm typecheck` | ✅ limpo |
| `pnpm lint` (biome + rustfmt + clippy `-D warnings`) | ✅ limpo |
| `pnpm build` (tsc + vite) | ✅ 301 kB JS / 44 kB CSS |
| `cargo build -p aisense-app` (janela Tauri) | ✅ compila (deps de sistema instaladas) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ limpo, agora incluindo o app |

As dependências de GUI foram instaladas no ambiente, então `aisense-app` **compila e passa no
clippy**. O que continua sem verificação é o comportamento visual: não há tela aqui, então a janela
nunca foi aberta. Em Linux as deps são
`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev patchelf`;
em macOS e Windows basta o toolchain.

**Nada visual foi conferido por um par de olhos.** Tema, layout, ausência de flash, anel de foco e
navegação por teclado estão implementados conforme a especificação, mas ninguém abriu a janela.
Quem tiver uma máquina com GUI: rode `pnpm dev` e abra `http://localhost:5173/#/dev` para ver a
amostra do design system nos dois temas — isso já valida a maior parte sem precisar do Tauri.

**Restam na Fase 00:** F00-02, F00-04 e F00-09, todas bloqueadas pela mesma coisa — precisam de um
ambiente com GUI ou de uma execução do CI.

**Fase 01 — Terminal Core.** Tudo escrito. O que dava para verificar, está verificado; o que
depende de janela, não.

| Camada | Situação |
|---|---|
| PTY, ring buffer, log, coalescência, gerenciador (Rust) | ✅ 48 testes, processos reais |
| Tipos de transporte e conversão de cor | ✅ testados |
| Comandos Tauri (casca fina) | ✅ compilam e passam no clippy |
| `<Terminal />`, tema, reidratação, busca | ⚠️ escritos, **nunca executados** |

Três decisões desta fase que valem lembrar:
1. O `PtyManager` mora em `aisense-pty`, não no crate do Tauri, para a lógica com estado e
   concorrência ser testável sem abrir janela. O app ficou como casca.
2. Os bytes do PTY trafegam em **base64**, não como texto: um caractere UTF-8 pode ser partido
   entre duas leituras e virar `�` se convertido cedo demais. Há teste que demonstra a corrupção
   do jeito ingênuo.
3. Tipo que vira TypeScript **nunca** mora em `aisense-app` — senão `pnpm gen:types` passa a exigir
   compilar a janela. Virou a regra R5 em `AGENTS.md`.

**Uma corrida que só o CI pegou.** A suíte passava 100% localmente e quebrou no runner do GitHub.
Eram dois defeitos somados, ambos de ordenação, e nenhum deles era "flake":

1. O receptor do canal `broadcast` era criado **depois** da thread de leitura começar. Um canal
   broadcast descarta em silêncio o que é enviado sem receptores inscritos, então a saída de um
   processo rápido (`echo`) se perdia. Agora o receptor principal nasce dentro do `spawn`, antes da
   leitura, e é entregue uma única vez por `take_output()`.
2. O evento de término era ordenado por **tempo** (`sleep(50ms)` depois do processo morrer) na
   esperança de que a saída já tivesse sido drenada. Sob carga não bastava, e a última linha —
   normalmente a mensagem de erro que explica a falha — chegava depois do painel já marcado como
   parado. Agora a ordem é **causal**: o término só é anunciado depois do EOF da leitura e da
   drenagem completa.

Guarda de regressão: `nao_perde_a_saida_de_um_processo_instantaneo` roda o cenário 10 vezes.
A suíte foi executada 20 vezes seguidas sem falha antes de reenviar.

Bugs encontrados pelos próprios testes, todos corrigidos na origem:
1. O corte de 64 KB do ring buffer só valia ao *continuar* uma entrada; um `push` único e grande
   virava uma entrada monolítica.
2. O `⌘F` estava num `onKeyDown` do elemento de fora, que quase nunca dispararia — o xterm move o
   foco para um textarea escondido. Passou para o `attachCustomKeyEventHandler`.
3. O teste do log falhou porque o PTY traduz `\n` em `\r\n` (termios `ONLCR`). O teste estava
   errado, não o código — mas a armadilha ficou documentada, porque vai morder de novo no detector
   de estado da Fase 03.

## Decisões já tomadas (não reabrir sem ADR)

- Desktop com **Tauri 2**, não Electron, não web puro → [ADR 0001](adr/0001-tauri-em-vez-de-electron.md)
- Core de domínio, PTY e barramento em **Rust** → [ADR 0002](adr/0002-core-em-rust.md)
- Persistência **SQLite local-first**, sem backend em nuvem no v1 → [ADR 0003](adr/0003-sqlite-local-first.md)
- Barramento em **NDJSON sobre socket local** (UDS/named pipe) → [ADR 0004](adr/0004-protocolo-do-barramento.md)
- Skills em **Markdown + frontmatter**, compatíveis com Claude Code → [ADR 0005](adr/0005-skills-markdown.md)
- **Os 4 itens da decisão D6 entram no v1** (aprovado pelo usuário em 2026-09-22): Notas da equipe
  → [15](15-notas-da-equipe.md), Bancadas → [16](16-bancadas.md), Comandos do projeto
  → [17](17-comandos-do-projeto.md), Gate de revisão → [13](13-quadro-kanban.md#gate-de-revisão).
  Viraram tarefas nas Fases 02, 04, 05 e 06 — não são fase nova.
- **Quadro Kanban é subsistema de primeira classe**, um por equipe, com API completa para agentes
  (`claim` atômico, WIP aplicado, automações de conjunto fechado) → [13](13-quadro-kanban.md)
- Entrega de mensagens **híbrida**: caixa de entrada + injeção opcional no PTY → [ADR 0006](adr/0006-entrega-de-mensagens.md)
- Front em **React 19 + TypeScript + Tailwind 4 + Radix**, terminal com **xterm.js/WebGL**

## Decisões pendentes

| # | Questão | Quando decidir | Opção padrão se ninguém decidir |
|---|---|---|---|
| D1 | Suporte a agentes remotos (SSH / container) | Fase 7 | Fora do v1; arquitetura já deixa o `AgentRuntime` plugável |
| D2 | Rastreio de custo por agente (tokens/USD) | Fase 8 | Só o que o próprio runtime imprimir no terminal; sem estimativa própria |
| D3 | Marketplace de skills | Pós-v1 | Import/export de pasta `.zip` apenas |
| D4 | Modo daemon headless (usar AISENSE sem GUI) | Pós-v1 | O crate `aisense-ipc` já é separado justamente para permitir isso depois |
| D5 | Telemetria anônima | Fase 9 | Desligada por padrão, opt-in explícito |


## Riscos ativos

| Risco | Impacto | Mitigação | Dono |
|---|---|---|---|
| Detecção de "agente ocioso" por heurística de prompt falhar em algum runtime | Alto — injeção de mensagem no meio de uma resposta | Fila de entrega + modo pull como padrão; regex por adaptador; ver [ADR 0006](adr/0006-entrega-de-mensagens.md) | — |
| WebKitGTK no Linux renderizar diferente do WebView2/WKWebView | Médio | CI com screenshot nos 3 SOs desde a Fase 0; evitar CSS de ponta | — |
| CLIs de terceiros (claude/codex/opencode) mudarem flags | Médio | Adaptadores em TOML, editáveis pelo usuário sem recompilar | — |
| Performance com 9+ terminais simultâneos | Médio | Ring buffer no Rust, render só do visível, coalescência a 60fps | — |

## Log de sessões

| Data | Quem | O que fez |
|---|---|---|
| 2026-09-22 | Claude | Levantamento inicial, definição de stack e escrita de toda a documentação base (docs/ 01–12, 6 ADRs, 9 fases) |
| 2026-09-22 | Claude | Análise do Maestri (referência do usuário); Kanban promovido a subsistema próprio (doc 13 + Fase 06 dedicada); fases 06–08 renumeradas para 07–09; backlog de software house documentado (doc 14) |
| 2026-09-22 | Claude | Início da Fase 00: scaffold do monorepo |
| 2026-09-22 | Claude | Vocabulário próprio (fim da metáfora musical: `@maestro` → `@coordenador`); F00-03, F00-06 e F00-07 concluídas |
| 2026-09-22 | Claude | Fase 01: PTY, ring buffer, log e coalescência em Rust, com 33 testes |
| 2026-09-22 | Claude | Fase 01: gerenciador de sessões, ponte Tauri, paleta ANSI testada, `<Terminal />` com xterm.js. 62 testes Rust + 99 front |
| 2026-09-22 | Claude | PR #3 aberto. CI vermelho revelou uma corrida na entrega de saída do PTY (perda silenciosa + ordem do evento de término); corrigida na origem com teste de regressão |
| 2026-09-22 | Claude | CI dos 3 SOs pegou erro de compilação no `aisense-app` (conversão de erro) e ícones ausentes. Deps de GUI instaladas no ambiente: o app agora compila e é lintado localmente |
| 2026-09-22 | Claude | D6 aprovada: docs 15, 16 e 17 escritos, gate de revisão no doc 13, e 6 tarefas novas distribuídas pelas Fases 02, 04, 05 e 06. Início da Fase 01 |
| 2026-09-22 | Claude | Início da Fase 02 (a pedido do usuário, com 00/01 pendentes só de verificação visual). F02-01 concluída |
