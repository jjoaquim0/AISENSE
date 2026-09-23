# FASE 02 — Equipes e Agentes

**Objetivo:** modelar e persistir o domínio — criar equipes, criar agentes, escolher runtime,
e ver tudo sobreviver ao fechar o app.

**Demonstração:** criar a equipe "Squad Produto" com 3 agentes de runtimes diferentes, fechar o app,
reabrir e encontrar tudo no lugar; iniciar um agente e ver o runtime certo subindo.

**Leitura obrigatória:** [04 — Modelo de Dados](../04-modelo-de-dados.md) ·
[05 — Adaptadores](../05-adaptadores-runtime.md) · [09 — Telas](../09-telas-e-fluxos.md)

## Tarefas

### [x] F02-01 — Modelos de domínio
`Team`, `Agent`, `AgentState`, `DeliveryMode`, `RestartPolicy`, IDs tipados (ULID), validações
(handle `^[a-z][a-z0-9-]{1,31}$`, unicidade na equipe). Traits de repositório definidas no core.
**Aceite:** testes de validação cobrindo handles inválidos, duplicados e reservados (`all`, `voce`).
> Feito: `aisense-core/src/{agent,team,repo}/`. Além do pedido: `Handle::suggest` (nome → handle,
> sem acento) para o T5, `AgentColor::next_free`, `RestartPolicy::should_restart`, transições da
> máquina de estados, e `InMemoryStore` implementando as portas com cascata e unicidade.
> Os enums de bancada (`WorkspaceMode`, `Workbench`, doc 16) já entram no modelo.

### [x] F02-02 — Store SQLite e migrações
`aisense-store`: pool SQLx, WAL, migração `0001_initial.sql` com o esquema de
[04 — Modelo de Dados](../04-modelo-de-dados.md), runner de migração na subida do app.
**Aceite:** banco criado do zero e migração idempotente; teste em `:memory:`. Depende de F02-01.
> Feito: `aisense-store/src/db.rs` + `migrations/0001_initial.sql` e `0002_workbenches.sql`.
> O app abre `~/.aisense/aisense.db` (`AISENSE_HOME` sobrescreve) no `setup` do Tauri, antes da
> janela. Backup automático antes de migrar, conforme o risco listado abaixo.

### [x] F02-03 — Repositórios de equipe e agente
Implementações de `TeamRepository` e `AgentRepository` (CRUD, listagem, arquivamento, cascade).
**Aceite:** testes de integração cobrindo cascade ao apagar equipe e a constraint de handle único.
Depende de F02-02.
> Feito: `aisense-store/src/{teams,agents}.rs`. `tests/repositories.rs` roda o **mesmo contrato**
> contra o SQLite e o `InMemoryStore` do core, para os dois nunca divergirem. Dado inválido no banco
> (enum desconhecido, handle reservado editado à mão) vira `RepoError::Corrupt`, nunca entidade.

### [x] F02-04 — Carregador de adaptadores
Parse dos TOMLs (embutidos + `~/.aisense/adapters/`), precedência do usuário, validação de schema,
hot-reload com `notify`.
**Aceite:** TOML inválido não derruba o app — é reportado com o caminho e a linha do erro.
> Feito: `aisense-core/src/adapter/`. `AdapterCatalog` sempre carrega: arquivo ruim vira
> `AdapterProblem` (`caminho:linha:coluna: mensagem`) e os outros seguem. `deny_unknown_fields`
> em todo o formato — erro de digitação num campo vira aviso, não campo ignorado. Regex de estado
> são compilados na carga. Embutidos vêm de `adapters/*.toml` na raiz via `include_str!`; por ora
> só `shell` (os demais são F02-07). `AdapterWatcher` recarrega com debounce de 200 ms.
> A ligação com o app (estado Tauri + evento para a UI) fica para F02-05, que é quem expõe a lista.

### [ ] F02-05 — Detecção de runtimes instalados
Executar o `detect` de cada adaptador (com timeout de 3 s, em paralelo), cachear o resultado,
expor versão e `install_hint`.
**Aceite:** a UI lista runtimes disponíveis e indisponíveis com a dica de instalação.
Depende de F02-04.

### [ ] F02-06 — Supervisor de agentes
`AgentSupervisor`: `start`/`stop`/`restart`, montagem do ambiente (variáveis do AISENSE + PATH dos
sidecars), registro de `sessions`, política de reinício com backoff exponencial, `CancellationToken`.
**Aceite:** matar o processo externamente dispara a política de reinício corretamente; `never` não
reinicia. Depende de F02-04, F01-01.

### [ ] F02-07 — Adaptadores embutidos
TOMLs de `claude`, `codex`, `opencode`, `gemini`, `shell` e `custom`, com regex de estado iniciais.
**Aceite:** cada um sobe um processo real (quando instalado) e o shell funciona em qualquer máquina.
Depende de F02-04.

### [ ] F02-08 — Telas de equipe
T2 (lista de equipes), T3 (assistente de criação com modelos de equipe), arquivar e excluir com
confirmação que exige digitar o nome.
**Aceite:** criar equipe pelo modelo "Dupla Dev" gera os 2 agentes configurados. Depende de F02-03.

### [ ] F02-09 — Painel de agente (T5)
Formulário completo de criação/edição com `react-hook-form` + `zod`, geração de handle a partir do
nome, seletor de runtime mostrando disponibilidade, atribuição de cor e seção "Avançado".
**Aceite:** validação impede handle duplicado antes de submeter; editar agente rodando avisa que
exige reinício. Depende de F02-05, F02-03.

### [ ] F02-10 — Bancadas (`git worktree` por agente)
Modos `shared` e `per-agent` por equipe, com exceção por agente. Criação, reutilização e validação
do worktree; fallback para `shared` com aviso quando o diretório não é git ou o Git é antigo.
Cópia dos arquivos ignorados declarados em `aisense.toml` e execução do `bench.setup`.
Remoção sempre por `git worktree remove`, recusando quando há mudanças não commitadas.
Ver [16 — Bancadas](../16-bancadas.md).
**Aceite:** dois agentes de uma equipe `per-agent` editam o mesmo arquivo sem se atropelar; excluir
um agente com trabalho pendente é recusado com mensagem clara. Depende de F02-06.

### [ ] F02-11 — `aisense.toml` (comandos do projeto)
Parser, validação e detecção automática que **propõe** um arquivo (nunca cria sozinho) a partir de
`pnpm-lock.yaml`, `Cargo.toml`, `pyproject.toml` ou `Makefile`.
Ver [17 — Comandos do Projeto](../17-comandos-do-projeto.md).
**Aceite:** TOML inválido é reportado com caminho e linha sem derrubar o app; a proposta aparece na
UI com o conteúdo gerado para revisão.

## Critérios de saída
- [ ] CRUD completo de equipes e agentes, persistido
- [ ] Runtimes detectados e adaptadores carregando de disco
- [ ] Agente sobe com o comando e o ambiente corretos
- [ ] Política de reinício funcionando
- [ ] Modelos de equipe criando squads prontos
- [ ] Bancadas isolando agentes em `per-agent`, com fallback seguro quando não há git
- [ ] `aisense.toml` lido e proposto automaticamente

## Riscos
| Risco | Mitigação |
|---|---|
| Migração futura quebrar banco de usuário | Migrações só aditivas; backup automático do `.db` antes de migrar |
| `detect` travar se a CLI abrir prompt interativo | Timeout de 3 s e `stdin` fechado |
