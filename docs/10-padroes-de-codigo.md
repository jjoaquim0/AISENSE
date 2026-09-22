# 10 — Padrões de Código

## Idioma

| Onde | Idioma |
|---|---|
| Texto exibido na interface | pt-BR |
| Código, identificadores, nomes de arquivo, tipos | inglês |
| Comentários de código | inglês |
| Mensagens de commit e PR | inglês |
| Documentação em `docs/` | pt-BR |
| Skills embutidas | pt-BR (são lidas por IA a serviço de um usuário brasileiro) |

## Rust

### Estrutura de módulo
Um conceito por arquivo. `mod.rs` só reexporta. Nada de arquivo de 2.000 linhas.

```rust
// crates/aisense-core/src/agent/mod.rs
mod model;
mod supervisor;
mod state;
pub use model::{Agent, AgentId, DeliveryMode};
pub use supervisor::AgentSupervisor;
pub use state::AgentState;
```

### Erros
`thiserror` por crate, um enum por domínio. **Nunca** `anyhow` em biblioteca (só em binários).

```rust
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("agent @{0} not found in this team")]
    UnknownAgent(String),
    #[error("agent @{0} is stopped")]
    AgentStopped(String),
    #[error("no reply from @{agent} within {secs}s")]
    Timeout { agent: String, secs: u64 },
    #[error("rate limit exceeded: {limit} messages/min")]
    RateLimited { limit: u32 },
}
```

Na fronteira do Tauri, converta para um payload serializável com `code` + `message` + `hint`.
A UI decide o que mostrar; o `hint` é sempre acionável ("inicie o agente" e não "erro interno").

### Proibições
- `unwrap()` / `expect()` fora de `main`, testes e inicialização que deve mesmo abortar.
- `panic!` em código de biblioteca.
- Bloquear a thread em runtime async (use `spawn_blocking`).
- Segurar `RwLockGuard` atravessando `.await`.
- `println!` — use `tracing::{info, debug, warn, error}`.

### IDs tipados
Nada de `String` solta para identificador:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct AgentId(String);
```

Vale para `TeamId`, `AgentId`, `SkillId`, `MessageId`, `TaskId`, `SessionId`.

### Testes
- Domínio (`core`): unitários puros, sem I/O. Repositórios são traits → `InMemoryStore` nos testes.
- `store`: testes de integração com SQLite em `:memory:`, migrações aplicadas.
- `pty`: testes com comandos determinísticos (`echo`, `cat`, `sleep`).
- Snapshots de composição do `BOOT.md` com `insta`.
- Meta de cobertura no `core`: **≥80%**. Nos demais crates, cobrir os caminhos de erro.

## TypeScript / React

### Organização por feature
```
src/features/team-room/
├── TeamRoom.tsx          # composição da tela
├── views/                # GridView, FocusView, FlowView, TimelineView
├── components/           # AgentPane, PaneHeader, MiniPreview
├── hooks/                # useTeamRoom, usePaneLayout
├── api.ts                # wrappers de invoke() — único lugar que chama Tauri
└── store.ts              # slice zustand da feature
```

**Nenhum componente chama `invoke()` direto.** Sempre via `api.ts` da feature, que retorna tipos
gerados por `ts-rs`.

### Regras de componente
- Function components com tipos explícitos de props. Sem `React.FC`.
- Props de dado primeiro, handlers depois, `className` por último.
- Sem `any`. `unknown` + narrowing quando necessário. `strict: true` no tsconfig.
- Sem cor, tamanho ou fonte literal — só classes de token.
- Um componente que passa de ~150 linhas provavelmente são dois componentes.
- Efeitos com dependências corretas; nada de `// eslint-disable-next-line react-hooks/exhaustive-deps`
  sem comentário explicando.

### Estado
| Tipo de estado | Onde mora |
|---|---|
| Servidor (vem do Rust) | `@tanstack/react-query`, invalidado por eventos Tauri |
| UI global (tema, vista, painel focado) | `zustand` |
| UI local | `useState` |
| Formulário | `react-hook-form` + `zod` |

### Eventos Tauri
Assinatura sempre em um único lugar por domínio (`src/lib/events.ts`), que traduz evento →
invalidação de query ou atualização de store. **Nunca** `listen()` espalhado por componente:
vaza listener e vira bug de memória com 9 terminais.

## Git

### Branches
`claude/epic-goldberg-mloopx` é a branch de desenvolvimento deste ciclo (ver `AGENTS.md`).
Trabalho novo sai dela.

### Commits — Conventional Commits com ID da tarefa
```
feat(F02-03): add team CRUD to SQLite store
fix(F05-07): prevent PTY injection while agent awaits confirmation
docs(F00): describe adapter TOML schema
refactor(F03-04): extract pane layout into hook
test(F01-05): cover ring buffer overflow
chore: bump xterm to 5.6
```
Tipos: `feat · fix · docs · refactor · test · perf · chore · build · ci`.
Título no imperativo, ≤72 caracteres. Corpo explica **por quê**, não o quê (o diff já diz o quê).

### Pull requests
Título com o ID da tarefa. Corpo com: o que muda, como testar, o que **não** foi feito,
e screenshot nos dois temas quando tocar em UI.

## Definition of Done (vale para toda tarefa)

Uma tarefa só é `[x]` quando:

1. ✅ Funciona no caminho feliz **e** nos caminhos de erro previstos
2. ✅ Tem testes (unitários no mínimo; integração se cruzar camadas)
3. ✅ `pnpm lint` e `pnpm test` passam localmente
4. ✅ Se mexe em UI: verificado em tema claro **e** escuro, e navegável por teclado
5. ✅ Se muda comportamento documentado: a doc foi atualizada no mesmo commit
6. ✅ Se tomou decisão arquitetural: existe um ADR
7. ✅ `docs/ESTADO.md` e o arquivo da fase foram atualizados
8. ✅ Sem `TODO` novo sem issue correspondente

## Registro (logging)

```rust
tracing::info!(agent = %agent.handle, state = ?new_state, "agent state changed");
```

Níveis: `error` (usuário precisa agir) · `warn` (degradação) · `info` (marco do ciclo de vida) ·
`debug` (detalhe de diagnóstico) · `trace` (fluxo de bytes — nunca ligado por padrão).

**Nunca registre**: conteúdo de mensagens em `info` (pode ter dado sensível), tokens, chaves de API,
variáveis de ambiente inteiras.
