# ESTADO DO PROJETO

> **Este é o arquivo mais importante do repositório para quem chega.**
> Toda sessão de desenvolvimento começa lendo isto e termina atualizando isto.
> Se estiver desatualizado, o próximo agente se perde. Mantenha-o honesto.

**Última atualização:** 2026-09-22
**Fase atual:** `FASE 00 — Fundação` (não iniciada)
**Branch de desenvolvimento:** `claude/epic-goldberg-mloopx`

---

## Situação em uma frase

A documentação completa de produto, arquitetura, design e plano de execução está escrita.
**Nenhuma linha de código de aplicação existe ainda.** O próximo passo é a Fase 0 (scaffold do monorepo).

## Progresso por fase

| Fase | Status | Tarefas |
|---|---|---|
| 00 — Fundação | 🟨 Em andamento | 3 feitas, 6 parciais de 9 |
| 01 — Terminal Core | ⬜ Não iniciada | 0/8 |
| 02 — Equipes e Agentes | ⬜ Não iniciada | 0/9 |
| 03 — Sala da Equipe | ⬜ Não iniciada | 0/9 |
| 04 — Sistema de Skills | ⬜ Não iniciada | 0/8 |
| 05 — Barramento | ⬜ Não iniciada | 0/11 |
| 06 — Quadro Kanban | ⬜ Não iniciada | 0/10 |
| 07 — Coordenação | ⬜ Não iniciada | 0/6 |
| 08 — Acabamento | ⬜ Não iniciada | 0/9 |
| 09 — Distribuição | ⬜ Não iniciada | 0/7 |

Legenda: ⬜ não iniciada · 🟨 em andamento · ✅ concluída · 🟥 bloqueada

## Em andamento agora

**Fase 00 — Fundação.** O esqueleto está de pé e verificado no que dava para verificar:

| Verificado aqui | Resultado |
|---|---|
| `cargo test --workspace --exclude aisense-app` | ✅ 13 testes |
| `pnpm --filter @aisense/desktop test --run` | ✅ 54 testes de contraste |
| `pnpm lint` (biome + rustfmt + clippy `-D warnings`) | ✅ limpo |
| `pnpm build` (tsc + vite) | ✅ 229 kB JS / 38 kB CSS |
| `cargo build -p aisense-app` (janela Tauri) | ⚠️ **não verificado** |

O ambiente de desenvolvimento usado não tem WebKit/GTK, então o crate `aisense-app` não compila
nele. Isso **não** indica problema no código: é dependência de sistema. Em Linux instale
`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev patchelf`;
em macOS e Windows não é preciso nada além do toolchain. O job `desktop` do CI faz essa verificação.

**Próximo passo:** fechar as 5 tarefas parciais da Fase 00 (ver as notas `>` em cada uma) —
em especial F00-06 (resto dos componentes) e F00-07 (inspetor e redimensionamento).

## Decisões já tomadas (não reabrir sem ADR)

- Desktop com **Tauri 2**, não Electron, não web puro → [ADR 0001](adr/0001-tauri-em-vez-de-electron.md)
- Core de domínio, PTY e barramento em **Rust** → [ADR 0002](adr/0002-core-em-rust.md)
- Persistência **SQLite local-first**, sem backend em nuvem no v1 → [ADR 0003](adr/0003-sqlite-local-first.md)
- Barramento em **NDJSON sobre socket local** (UDS/named pipe) → [ADR 0004](adr/0004-protocolo-do-barramento.md)
- Skills em **Markdown + frontmatter**, compatíveis com Claude Code → [ADR 0005](adr/0005-skills-markdown.md)
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
| D6 | Incluir no v1 os 4 itens 🔴 de [14](14-ideias-software-house.md) (Notas da equipe, Bancadas/worktree, Comandos do projeto, Gate de revisão) | Antes da Fase 04 | **Aguardando decisão do usuário.** Se ninguém decidir, vão para o v1.1 e o v1 sai sem eles |

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
