# ESTADO DO PROJETO

> **Este é o arquivo mais importante do repositório para quem chega.**
> Toda sessão de desenvolvimento começa lendo isto e termina atualizando isto.
> Se estiver desatualizado, o próximo agente se perde. Mantenha-o honesto.

**Última atualização:** 2026-09-23
**Fase atual:** `FASE 04 — Sistema de Skills` (iniciada a pedido do usuário; a 03 tem código completo e dois critérios de saída abertos)
**Fluxo de trabalho atual:** um PR por tarefa, a partir de um branch `claude/...`, com o CI verde
nos 3 SOs antes do merge (o usuário mergeia). Commits com o ID da tarefa no título.

---

## Situação em uma frase

Fundação, terminal, equipes/agentes persistidos e **as 9 tarefas da Sala da Equipe** estão em
código e testados; a Fase 03 ainda tem conferência na janela e dois critérios de saída abertos
(9 terminais com GPU; calibrar o detector com sessões reais). O trabalho corrente é a
**Fase 04 — Skills**, iniciada a pedido do usuário.

## Para quem chega agora (retomada)

1. Leia `AGENTS.md` e este arquivo inteiro; depois `docs/fases/FASE-03-sala-da-equipe.md` — cada
   tarefa feita tem uma nota `> Feito:` dizendo onde está o código e o que ficou de fora.
2. Próxima tarefa: **F04-03** (resolução das skills de um agente no boot), em
   `docs/fases/FASE-04-skills.md`. Skills em `aisense-core/src/skill/` (parser, catálogo,
   `SkillLibrary`, hot-reload); atribuições por `SkillRepository`; aba Skills do inspetor em
   `features/team-room/components/inspector/SkillsTab.tsx`. Onde a Fase 03 deixou as coisas: sidebar e inspetor entram no shell por `ShellSlot`
   (`features/shell/slots.tsx`); o inspetor tem as abas Visão/Config/Logs e espera Skills
   (Fase 04) e Caixa (Fase 05); a sidebar reordena a **ordem da equipe** (`agents.position`);
   atalhos em `useShortcuts` + ADR 0007; transcrições em `aisense-core/src/transcript.rs`.
3. Onde as coisas moram na Fase 03: `aisense-core/src/state/` (detector), `supervisor/team.rs`
   (▶ ⏸ ⟳), `apps/desktop/src/features/team-room/` (painel, grid, foco, controles, layout).
4. Verificação que se espera antes de cada PR: `pnpm lint`, `pnpm typecheck`, `pnpm test`,
   `pnpm build` e `cargo clippy -p aisense-app --all-targets -- -D warnings` (no Linux precisa das
   libs de GUI, ver `AGENTS.md`). UI nova: screenshot em `#/dev` nos dois temas (Chromium headless
   funciona aqui); desempenho da grade: `#/dev/grid`.
5. **Pendências que só uma máquina com tela resolve:** demonstração da Fase 02 na janela;
   conferência visual das Fases 00/01; 55 fps com 9 terminais ativos (`#/dev/grid` com GPU);
   barra de progresso e confirmação dos controles da equipe na janela real.

## Progresso por fase

| Fase | Status | Tarefas |
|---|---|---|
| 00 — Fundação | 🟨 Em andamento | 6 feitas, 3 parciais de 9 |
| 01 — Terminal Core | 🟨 Em andamento | 4 feitas, 4 parciais de 8 |
| 02 — Equipes e Agentes | 🟨 Código completo | 11 de 11 feitas — falta a demonstração na janela |
| 03 — Sala da Equipe | 🟨 Código completo | 9 de 9 feitas — faltam conferência na janela e dois critérios de saída |
| 04 — Sistema de Skills | 🟨 Em andamento | 2 feitas de 9 |
| 05 — Barramento | ⬜ Não iniciada | 0/13 |
| 06 — Quadro Kanban | ⬜ Não iniciada | 0/11 |
| 07 — Coordenação | ⬜ Não iniciada | 0/6 |
| 08 — Acabamento | ⬜ Não iniciada | 0/9 |
| 09 — Distribuição | ⬜ Não iniciada | 0/7 |

Legenda: ⬜ não iniciada · 🟨 em andamento · ✅ concluída · 🟥 bloqueada

## Em andamento agora

**Fase 04 — Sistema de Skills** (iniciada em 2026-09-23 a pedido do usuário). Exceção consciente
à R1: a Fase 03 tem as 9 tarefas em código, mas faltam a conferência na janela e dois critérios de
saída que só uma máquina com GPU e os runtimes reais resolvem. Nada da Fase 04 depende deles.

| Tarefa | Situação |
|---|---|
| F04-01 Parser e validador | ✅ `SKILL.md` com `caminho:linha` nos erros, campos desconhecidos ignorados (compatível com Claude Code), catálogo embutidas + `~/.aisense/skills/`; `yaml-rust2` no lugar do `gray_matter` |
| F04-02 Registro e persistência | ✅ `skills`/`agent_skills` em SQLite e memória (mesmo contrato), sync sem apagar o que sumiu do disco, hot-reload recursivo com `skills:changed`, aba Skills do inspetor |

**Fase 03 — Sala da Equipe** (iniciada em 2026-09-23 a pedido do usuário). Exceção consciente à R1:
as 11 tarefas da Fase 02 estão feitas, mas a **demonstração na janela** (criar "Squad Produto",
fechar, reabrir, subir agente) ainda não foi feita — o usuário estava sem o computador. Continua
pendente e não bloqueia nada da Fase 03.

| Tarefa | Situação |
|---|---|
| F03-01 Detector de estado | ✅ `StateDetector` (tela via `vt100`, regras do `docs/05`), tarefa por sessão no supervisor, `agent:state` com `confidence` |
| F03-02 Painel de agente | ✅ `AgentPane` com borda na cor do agente, estado com forma + texto, menu `⋮` (reiniciar, parar/iniciar, limpar, duplicar, configurar) |
| F03-03 Vista Grid | ✅ presets 1/2/3/4/6/9 + livre, arrastar/redimensionar, layout salvo por equipe; 60 fps arrastando (terminais parados, sem GPU) — 9 terminais ativos a 55 fps só verificável com GPU |
| F03-04 Vista Foco | ✅ terminal grande + miniaturas de texto (sem xterm) vindas da tela do detector, a 2 fps; vista salva por equipe |
| F03-05 Visibilidade | ✅ agentes nascem invisíveis; só o painel na tela (e com a janela em primeiro plano) recebe `pty:data`; reidratação ordenada ao voltar |
| F03-06 Controles da equipe | ✅ ▶ escalonado (300 ms), ⏸ e ⟳ com confirmação e progresso ao vivo (`team:progress`) |
| F03-07 Sidebar de agentes | ✅ lista na sidebar do shell (portal), estado direto do evento, reordenar grava `position` (`agents_reorder`), duplo clique abre o inspetor (cabeçalho do agente; abas na F03-09), badge de mensagens pronto para a Fase 05 |
| F03-08 Navegação por teclado | ✅ camada única na captura da janela; ⌘1..9 valem com o terminal focado sem vazar para o shell; ⌘G, ⌘T, ⌘W (fecha o painel, não o agente), ⌘\\, Esc Esc; fora do macOS o resto fica com o shell ([ADR 0007](adr/0007-atalhos-com-o-terminal-focado.md)) |
| F03-09 Inspetor do agente | ✅ abas Visão (estado, tempo ativo, PID, últimos eventos), Config (campos do T5 inline) e Logs (transcrição por sessão com busca e exportação; `sessions.log_offset`, migração 0003) |

O agente agora fica em `starting` até o detector ler a primeira tela — antes, processo vivo era
`idle` na hora. Transcrições dos testes são sintéticas; ver ressalva no documento da fase.

**CI da main estava vermelho desde a F02-08** (job do Windows, teste
`grava_a_transcricao_completa_em_arquivo`: esperava 200 ms fixos depois do processo sair, e o
ConPTY ainda entrega saída depois disso). A causa real era de produto: o console era fechado no
instante em que o processo morria, descartando as últimas linhas (em geral a mensagem de erro).
Agora espera a leitura silenciar antes de fechar. PR #5, mergeado.


**Fase 02 — Equipes e Agentes** (iniciada em 2026-09-22 a pedido do usuário). Exceção consciente à
regra R1: o que resta das Fases 00 e 01 é **só verificação visual/CI** (F00-02, F00-04, F00-09,
F01-05..08), nada de código faltando, e nada da Fase 02 depende disso. As pendências continuam
abertas e devem ser fechadas por quem tiver uma máquina com tela.

| Tarefa | Situação |
|---|---|
| F02-01 Modelos de domínio | ✅ `Team`, `Agent`, `Handle`, políticas, estados, portas de repositório e `InMemoryStore` |
| F02-02 Store SQLite e migrações | ✅ `Store::open` com WAL, FKs, backup antes de migrar; o app migra no `setup` |
| F02-03 Repositórios | ✅ equipe e agente sobre SQLite; mesmo contrato testado contra SQLite e memória |
| F02-04 Carregador de adaptadores | ✅ `AdapterCatalog` (embutidos + pasta do usuário, erro com `caminho:linha`) e `AdapterWatcher` (hot-reload) |
| F02-05 Detecção de runtimes | ✅ `RuntimeRegistry` (detect em paralelo, timeout 3 s, cache), comando `runtimes_overview`, lista na tela inicial — vista na janela |
| F02-07 Adaptadores embutidos | ✅ claude, codex, opencode (flags conferidas nas versões instaladas), gemini (não conferido), shell, custom |
| F02-06 Supervisor | ✅ start/stop/restart, ambiente `AISENSE_*`, sessões, reinício com backoff; comandos `agent_*` e evento `agent:state` |
| F02-08 Telas de equipe | ✅ T2 (cards), T3 (assistente com modelos), arquivar e excluir digitando o nome |
| F02-09 Painel do agente | ✅ formulário T5 (react-hook-form + zod), CRUD de agente, visão da equipe com terminal real do agente |
| F02-11 `aisense.toml` | ✅ parser com linha do erro, detecção que só propõe, botão "Comandos" com aceite que nunca sobrescreve |
| F02-10 Bancadas | ✅ `git worktree` por agente com fallback para `shared`, cópia do `bench.copy`, `bench.setup`, remoção recusada com pendências |

Verificado aqui: `pnpm lint` e `pnpm test` limpos (62 core + 49 pty + 22 store + 99 front),
`cargo clippy -p aisense-app -- -D warnings` e `cargo build -p aisense-app` ok.

**Primeira abertura da janela (2026-09-22, Windows):** o app sobe, a tela inicial aparece e a lista
de runtimes funciona — conferido pelo usuário. Para isso foi preciso criar
`crates/aisense-app/capabilities/default.json`: sem ele o Tauri 2 nega `listen`, e nenhum evento do
core (nem `pty:data`) chegaria ao front. O resto da conferência visual das Fases 00/01 continua aberto.

**Próximas:** a demonstração da Fase 02 na janela (criar equipe, fechar, reabrir, subir agente) e
então a **Fase 03 — Sala da Equipe**. Já dá para criar uma equipe, subir agentes, usar o
terminal deles e isolá-los em bancadas pela interface.

Duas decisões desta fase que valem lembrar:
1. Os modelos serializam em **camelCase** para o front; o SQL segue snake_case e o store traduz.
2. `agents.env` não aceita chaves `AISENSE_*` — são elas que carregam a identidade do agente no
   barramento (invariante I7 em `docs/04`). O `[env]` dos adaptadores segue a mesma regra.
3. Adaptador com campo desconhecido é recusado (`deny_unknown_fields`): um typo em `idle_regex`
   desligaria o detector de estado em silêncio. Dependências novas no core: `toml`, `regex`
   (validar os padrões já na carga; o detector da Fase 03 vai usá-la) e `notify` (hot-reload,
   previsto em `docs/03`).


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
- Atalhos com o terminal focado: só `⌘1..9` fora do macOS, `Esc Esc` para sair → [ADR 0007](adr/0007-atalhos-com-o-terminal-focado.md)
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
| Detecção de "agente ocioso" por heurística de prompt falhar em algum runtime | Alto — injeção de mensagem no meio de uma resposta | Fila de entrega + modo pull como padrão; regex por adaptador; ver [ADR 0006](adr/0006-entrega-de-mensagens.md). O detector (F03-01) só foi testado com transcrições **sintéticas**: gravar sessões reais de cada runtime e ajustar os regex antes da Fase 05, quando o estado passa a decidir injeção | — |
| WebKitGTK no Linux renderizar diferente do WebView2/WKWebView | Médio | CI com screenshot nos 3 SOs desde a Fase 0; evitar CSS de ponta | — |
| CLIs de terceiros (claude/codex/opencode) mudarem flags | Médio | Adaptadores em TOML, editáveis pelo usuário sem recompilar | — |
| ConPTY nativo do Windows 10 rola só ~30 linhas/s (conhost antigo redesenha por linha) | Médio — saída longa (build, testes) aparece com atraso; TUIs que redesenham a tela sofrem menos | Decidir na Fase 09 se o instalador leva `conpty.dll` + `OpenConsole.exe` próprios (MIT, é o que o VS Code faz); o `portable-pty` já carrega um `conpty.dll` ao lado do executável | — |
| Cópia local do `portable-pty` em `vendor/` (flags do ConPTY e `kill` invertido corrigidos) | Baixo — fica para trás de atualizações do original | `vendor/portable-pty/AISENSE.md` diz o que mudou e quando remover; os testes do PTY rodam no Windows no CI | — |
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
| 2026-09-22 | Claude | Início da Fase 02 (a pedido do usuário, com 00/01 pendentes só de verificação visual). F02-01, F02-02 e F02-03 concluídas: domínio, SQLite com migrações e repositórios |
| 2026-09-22 | Claude | Ambiente Windows do usuário montado (rustup, MSVC, pnpm). F02-04: carregador de adaptadores com hot-reload |
| 2026-09-22 | Claude | F02-05: detecção de runtimes + lista na tela inicial; capabilities do Tauri criadas; primeira abertura da janela conferida pelo usuário |
| 2026-09-22 | Claude | F02-07: adaptadores embutidos; `which` deixa de achar o script sem extensão do npm no Windows |
| 2026-09-23 | Claude | PTY no Windows: 49/49 testes. ConPTY não dava EOF ao fim do processo, travava com as flags do `portable-pty` (cópia em `vendor/`) e o `kill` tinha resultado invertido. Testes do PTY no job Windows do CI |
| 2026-09-23 | Claude | F02-06: supervisor de agentes com sessões e política de reinício; biome passa a ignorar `vendor/` |
| 2026-09-23 | Claude | F02-08: modelos de equipe, criação atômica, T2 e T3 no front; pnpm 10.33 (o do `packageManager`) para não reescrever o lockfile |
| 2026-09-23 | Claude | F02-09: formulário do agente e visão da equipe com terminal; dependências novas no front: react-hook-form, zod, @hookform/resolvers (previstas na fase) |
| 2026-09-23 | Claude | F02-11: `aisense.toml` (parser, validação, detecção que propõe) e o diálogo "Comandos" |
| 2026-09-23 | Claude | F02-10: bancadas (git worktree por agente), integradas ao supervisor e às exclusões. Fase 02 com as 11 tarefas feitas |
| 2026-09-23 | Claude | CI vermelho no Windows desde a F02-08 diagnosticado e corrigido (PR #5). Início da Fase 03 a pedido do usuário; F03-01 (detector de estado) concluída |
| 2026-09-23 | Claude | Causa real do CI vermelho no Windows: ConPTY fechado cedo demais perdia as últimas linhas; corrigido. F03-02 (painel de agente com menu ⋮, duplicar e limpar) |
| 2026-09-23 | Claude | F03-03 (vista Grid com dnd-kit, modo livre, layout por equipe) e bancada de desempenho `#/dev/grid` |
| 2026-09-23 | Claude | F03-04 (vista Foco com miniaturas leves) e correção do preset inicial da Grid |
| 2026-09-23 | Claude | F03-05 (só o painel visível recebe saída ao vivo; agentes nascem invisíveis) |
| 2026-09-23 | Claude | F03-06 (iniciar/parar/reiniciar a equipe com progresso, sem travar a interface) |
| 2026-09-23 | Claude | Retomada documentada no topo deste arquivo para continuar em outra sessão; próxima tarefa F03-07 |
| 2026-09-23 | Claude | F03-07 (sidebar de agentes no shell, estado ao vivo sem ida ao core, reordenação persistida) |
| 2026-09-23 | Claude | F03-08 (atalhos de teclado com o terminal focado; ADR 0007) |
| 2026-09-23 | Claude | F03-09 (inspetor com Visão, Config e Logs; transcrição por sessão). Fase 03 com código completo |
| 2026-09-23 | Claude | Início da Fase 04 a pedido do usuário; F04-01 (parser e catálogo de skills) |
| 2026-09-23 | Claude | F04-02 (skills no banco, hot-reload da biblioteca, aba Skills do inspetor) |
