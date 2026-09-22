# AGENTS.md — Como trabalhar neste repositório

> Este arquivo é a **porta de entrada obrigatória** para qualquer agente de IA (ou pessoa) que for
> escrever código no AISENSE. Ele existe para que ninguém se perca: nenhuma sessão começa do zero,
> nenhuma decisão é reinventada, nenhum trabalho é duplicado.

## 1. Ritual de início de sessão (sempre, sem exceção)

Execute nesta ordem antes de escrever qualquer linha de código:

1. Leia **`docs/ESTADO.md`** — é o ledger vivo do projeto. Diz a fase atual, o que está pronto,
   o que está em andamento e quais são as decisões pendentes.
2. Leia o documento da **fase atual** em `docs/fases/FASE-XX-*.md`. Ele tem a lista de tarefas com IDs.
3. Escolha **uma tarefa** com status `[ ]` (não iniciada) e cujas dependências estejam `[x]`.
4. Leia os documentos listados no campo **"Leitura obrigatória"** daquela tarefa.
5. Só então implemente.

## 2. Ritual de fim de sessão (sempre, sem exceção)

1. Marque a tarefa em `docs/fases/FASE-XX-*.md` como `[x]` (feita) ou `[~]` (parcial, com nota do que falta).
2. Atualize a seção "Em andamento" e "Última atualização" de `docs/ESTADO.md`.
3. Se você tomou uma decisão técnica relevante que não estava documentada, crie um ADR em
   `docs/adr/NNNN-titulo.md` usando o template de `docs/adr/0000-template.md`.
4. Faça commit com o ID da tarefa no título: `feat(F02-03): CRUD de equipes no store SQLite`.

## 3. Regras invioláveis

| # | Regra |
|---|---|
| R1 | **Nunca pule fases.** Se a Fase 3 não terminou, não comece a Fase 5. Se precisar de algo de uma fase futura, crie um stub e registre em `ESTADO.md`. |
| R2 | **Documento vence código.** Se o código diverge da doc, ou você corrige o código, ou atualiza a doc **na mesma mudança**. Nunca deixe os dois em desacordo. |
| R3 | **Nada de segredo em disco simples.** Chaves de API vão no keychain do SO. Veja `docs/11-seguranca.md`. |
| R4 | **Nada de `unwrap()`/`expect()` em caminho de execução.** Erros usam `thiserror` no core e são convertidos na fronteira do Tauri. Exceção: inicialização e testes. |
| R5 | **Toda chamada de comando Tauri tem tipo compartilhado.** Os tipos vêm de `ts-rs` gerando TypeScript a partir do Rust; não escreva tipos duplicados à mão. E eles **nunca** moram em `aisense-app`: o `ts-rs` exporta durante os testes, então um tipo definido lá tornaria `pnpm gen:types` dependente de compilar a janela (no Linux, de ter WebKit/GTK). Tipo que vira TypeScript mora em `aisense-core` ou no crate do subsistema. |
| R6 | **A UI nunca bloqueia.** Qualquer coisa que dure >16 ms roda no core em Rust. |
| R7 | **Tudo tem tema claro e escuro.** Nenhuma cor literal em componente — só tokens semânticos. Veja `docs/08-design-system.md`. |
| R8 | **Textos de interface em pt-BR; código, identificadores, commits e nomes de arquivo em inglês.** |
| R9 | **Sem dependência nova sem justificativa.** Se adicionar uma crate/pacote, escreva o porquê no PR. Se for arquitetural, é ADR. |
| R10 | **Não desligue teste para ficar verde.** Teste que falha é bug — no código ou no teste. |

## 4. Mapa de onde mexer

```
aisense/
├── crates/
│   ├── aisense-core/      # domínio puro: modelos, bus, skills, tarefas. Sem Tauri, sem I/O de UI.
│   ├── aisense-pty/       # gerenciamento de PTY, ring buffer, detecção de estado
│   ├── aisense-store/     # SQLite, migrações, repositórios
│   ├── aisense-ipc/       # servidor de socket (UDS / named pipe) que atende a CLI e o MCP
│   ├── aisense-cli/       # binário `aisense` — injetado no PATH de cada terminal
│   ├── aisense-mcp/       # binário `aisense-mcp` — servidor MCP stdio
│   └── aisense-app/       # app Tauri: comandos, eventos, estado global. Camada fina.
├── apps/desktop/          # front-end React (Vite)
│   ├── src/features/      # um diretório por feature (teams, agents, terminal, skills, bus...)
│   ├── src/components/    # componentes do design system
│   └── src/styles/        # tokens.css + tema
├── skills/                # skills embutidas que acompanham o app
├── adapters/              # TOMLs de runtime (claude, codex, opencode, shell...)
└── docs/                  # esta documentação
```

Regra de dependência: `app → core → {pty, store, ipc}`. **`core` nunca importa `app`.**
O front-end só fala com o Rust por comandos e eventos Tauri — nunca por HTTP, nunca por arquivo.

## 5. Comandos do dia a dia

> Estes comandos passam a valer a partir da Fase 0. Antes disso o scaffold não existe.

```bash
pnpm install          # dependências do front
pnpm dev              # só o front (Vite), sem janela
pnpm app              # app completo em modo dev (Tauri)
pnpm build            # build do front
pnpm lint             # biome + rustfmt + clippy
pnpm test             # vitest + cargo test
cargo test -p aisense-core   # testes de um crate só
```

### Verificando o crate `aisense-app`

`pnpm lint` e `pnpm test` **excluem `aisense-app`** para funcionarem em qualquer máquina.
Esse crate precisa das bibliotecas de GUI do sistema. Se você mexeu nele, instale-as e compile —
senão só o CI vai descobrir que não compila:

```bash
# Linux (Debian/Ubuntu)
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev patchelf

cargo clippy -p aisense-app --all-targets -- -D warnings
cargo build -p aisense-app
```

No macOS e no Windows basta o toolchain do Rust.

## 6. Quando estiver em dúvida

- **Dúvida de produto** (o que construir): `docs/01-visao-produto.md` e `docs/09-telas-e-fluxos.md`.
- **Dúvida de arquitetura** (onde colocar): `docs/02-arquitetura.md`.
- **Dúvida de "por que assim"**: `docs/adr/`.
- **Dúvida de nome/conceito**: `docs/12-glossario.md`.
- **Se a resposta não estiver em lugar nenhum**: registre em `docs/ESTADO.md` na seção
  "Decisões pendentes", escolha a opção mais simples e siga. Não trave.
