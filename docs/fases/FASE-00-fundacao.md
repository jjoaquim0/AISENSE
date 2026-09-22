# FASE 00 — Fundação

**Objetivo:** ter um monorepo que compila, roda como app de desktop nos 3 SOs e já carrega o
design system, para que nenhuma fase seguinte perca tempo com encanamento.

**Demonstração:** `pnpm dev` abre uma janela AISENSE com a shell da interface (trilho, sidebar,
área principal vazia), alternando tema claro/escuro, com fontes corretas.

**Leitura obrigatória:** [02 — Arquitetura](../02-arquitetura.md) · [03 — Stack](../03-stack.md) ·
[08 — Design System](../08-design-system.md) · [10 — Padrões](../10-padroes-de-codigo.md)

## Tarefas

### [ ] F00-01 — Estrutura do monorepo
Criar o workspace Cargo com os 7 crates (só com `lib.rs` vazio e as dependências entre eles
declaradas conforme a regra `app → core → {pty, store, ipc}`) e o workspace pnpm com `apps/desktop`.
Configurar `rustfmt.toml`, `clippy.toml`, `biome.json`, `.editorconfig`, `.gitignore`.
**Aceite:** `cargo build --workspace` e `pnpm install` passam limpos.

### [ ] F00-02 — App Tauri iniciando
Configurar `tauri.conf.json` (identificador, janela 1440×900, mínimo 1024×640, allowlist mínima
conforme [11 — Segurança](../11-seguranca.md)) e o bootstrap do Vite + React 19 + TypeScript strict.
**Aceite:** `pnpm dev` abre a janela com "AISENSE" renderizado pelo React. Depende de F00-01.

### [ ] F00-03 — Tokens de design em CSS
`apps/desktop/src/styles/tokens.css` com todos os tokens primitivos e semânticos de
[08 — Design System](../08-design-system.md), nos dois temas. Configurar Tailwind 4 com `@theme`
mapeando os tokens. Empacotar Inter Variable e JetBrains Mono como WOFF2 local.
**Aceite:** uma página de amostra mostra a escala tipográfica e a paleta, e nenhuma cor literal
aparece fora de `tokens.css`. Depende de F00-02.

### [ ] F00-04 — Alternância de tema sem flash
Store zustand de tema (`claro`/`escuro`/`sistema`), script inline no `index.html` aplicando
`data-theme` antes da primeira pintura, listener de `prefers-color-scheme`.
**Aceite:** reabrir o app no tema escuro não pisca branco em nenhum momento. Depende de F00-03.

### [ ] F00-05 — Teste de contraste em CI
`contrast.test.ts` que percorre todos os pares semânticos (texto sobre fundo) nos dois temas e
falha abaixo de 4.5:1 (3:1 para ≥18px).
**Aceite:** o teste roda no CI e falha propositalmente se alguém escurecer `--fg-secondary`.
Depende de F00-03.

### [ ] F00-06 — Componentes base do design system
`Button`, `IconButton`, `Input`, `Badge`, `StatusDot`, `Tooltip`, `Dialog`, `DropdownMenu`, `Tabs`,
`ScrollArea`, `EmptyState`, `Kbd` sobre Radix + CVA.
**Aceite:** uma rota `/dev/kitchen-sink` (só em dev) mostra todos, e todos são navegáveis por
teclado com anel de foco visível. Depende de F00-03.

### [ ] F00-07 — Shell da aplicação
Layout global de [09 — Telas](../09-telas-e-fluxos.md#estrutura-global-da-janela): trilho de 48px,
sidebar redimensionável, área principal, inspetor colapsável, barra de título integrada.
Persistir tamanhos no localStorage.
**Aceite:** o layout responde ao redimensionamento sem scroll horizontal e os painéis colapsam por
atalho. Depende de F00-06.

### [ ] F00-08 — Ponte tipada Rust ⇄ TypeScript
Configurar `ts-rs`, um comando Tauri de exemplo (`app_info`) e o script
`pnpm gen:types` que roda o export e grava em `src/types/generated/`.
**Aceite:** alterar um struct em Rust e rodar `pnpm gen:types` atualiza o `.ts`; o CI falha se os
tipos gerados estiverem desatualizados. Depende de F00-02.

### [ ] F00-09 — CI nos 3 sistemas operacionais
GitHub Actions: matriz macOS/Ubuntu/Windows rodando `cargo fmt --check`, `cargo clippy -D warnings`,
`cargo test`, `biome ci`, `vitest run` e build do Tauri. Cache de cargo e pnpm.
**Aceite:** um PR de teste fica verde nos três SOs em menos de 15 min. Depende de F00-02.

## Critérios de saída
- [ ] `pnpm dev` abre o app nos 3 SOs
- [ ] Tema claro e escuro completos, sem flash, com contraste validado em CI
- [ ] Componentes base prontos e acessíveis
- [ ] Tipos compartilhados gerando automaticamente
- [ ] CI verde nos 3 SOs
- [ ] `docs/ESTADO.md` atualizado

## Riscos
| Risco | Mitigação |
|---|---|
| Build do Tauri no Linux exigir dependências de sistema | Documentar as libs necessárias no README e instalá-las no workflow de CI |
| Tailwind 4 com `@theme` ser novidade para quem implementa | Tokens ficam em CSS puro; Tailwind só os consome. Se atrapalhar, dá para usar CSS puro sem perder nada |
