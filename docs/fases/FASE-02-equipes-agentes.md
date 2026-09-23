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

### [x] F02-05 — Detecção de runtimes instalados
Executar o `detect` de cada adaptador (com timeout de 3 s, em paralelo), cachear o resultado,
expor versão e `install_hint`.
**Aceite:** a UI lista runtimes disponíveis e indisponíveis com a dica de instalação.
Depende de F02-04.
> Feito: `aisense-core/src/adapter/detect.rs`. `RuntimeRegistry` guarda catálogo + cache por
> adaptador (invalidado só quando `command`/`detect` mudam). `which` respeita `PATHEXT` no Windows
> (CLIs do npm são `.cmd`). No app: comando `runtimes_overview(refresh)` em `spawn_blocking` e
> evento `adapters:changed` do hot-reload. Na UI, `features/runtimes/RuntimeList` aparece na tela
> inicial (o bloco "Encontramos no seu sistema" da T1). Conferido na janela, no Windows.
> Junto: `crates/aisense-app/capabilities/default.json` — sem ele o Tauri 2 nega `listen` e nenhum
> evento do core (inclusive `pty:data`) chegaria ao front.

### [x] F02-06 — Supervisor de agentes
`AgentSupervisor`: `start`/`stop`/`restart`, montagem do ambiente (variáveis do AISENSE + PATH dos
sidecars), registro de `sessions`, política de reinício com backoff exponencial, `CancellationToken`.
**Aceite:** matar o processo externamente dispara a política de reinício corretamente; `never` não
reinicia. Depende de F02-04, F01-01.
> Feito: `aisense-core/src/supervisor/` (o core passa a depender de `aisense-pty`, como a regra
> `app → core → {pty, store, ipc}` permite). `launch.rs` monta comando/args/cwd/env como função
> pura; `backoff.rs` dobra de 1 s a 60 s e zera depois de 60 s estável; `token.rs` gera o
> `AISENSE_TOKEN` com 32 bytes do SO. Porta nova `SessionRepository` (SQLite + memória, mesmo
> contrato, retenção de 200 por agente). Testes com processos reais cobrem o aceite: kill externo
> reinicia em `on-crash`, `never` não reinicia, `exit 0` é respeitado, `stop` cancela reinício
> agendado. No app: `agent_start/stop/restart/state` e evento `agent:state`; fechar a janela
> cancela os reinícios **antes** de matar os processos.
> Até o detector de estado (Fase 03), processo vivo = `idle`. O bootstrap (skills, `BOOT.md`,
> flag de system prompt) e o registro no barramento (passos 2–4, 7–8 do fluxo 1) são das Fases
> 04 e 05. A validação do token pelo barramento e a tabela `agent_tokens` são da Fase 05.
> Bug corrigido no caminho: o PTY não funcionava no Windows (ver ESTADO, 2026-09-23).

### [x] F02-07 — Adaptadores embutidos
TOMLs de `claude`, `codex`, `opencode`, `gemini`, `shell` e `custom`, com regex de estado iniciais.
**Aceite:** cada um sobe um processo real (quando instalado) e o shell funciona em qualquer máquina.
Depende de F02-04.
> Feito: `adapters/*.toml`. Flags de claude (2.1.260), codex (0.152.1) e opencode (1.18.27)
> conferidas no `--help` das versões instaladas; **gemini não foi verificado** (não instalado).
> O teste `installed_builtin_clis_answer_their_detect` roda o `detect` de verdade de toda CLI
> embutida presente na máquina — flag errada quebra o teste onde a CLI existir. O `custom` usa
> `command = "$AGENT_COMMAND"` e tem estado de detecção próprio (`perAgent`). Subir cada runtime
> **dentro de um PTY** é exercitado pelo supervisor (F02-06). Os regex de estado são iniciais; a
> calibração é da Fase 03.

### [x] F02-08 — Telas de equipe
T2 (lista de equipes), T3 (assistente de criação com modelos de equipe), arquivar e excluir com
confirmação que exige digitar o nome.
**Aceite:** criar equipe pelo modelo "Dupla Dev" gera os 2 agentes configurados. Depende de F02-03.
> Feito. Core: `team/templates.rs` (5 modelos; cada agente tem runtimes em ordem de preferência e
> cai para o próximo instalado) e `team/setup.rs` (`create_team_with_agents` valida tudo antes de
> gravar e desfaz a equipe se um agente falhar; `confirm_deletion`). O aceite é teste do core.
> App: `teams_list`, `team_template_plan`, `team_create`, `team_set_archived`, `team_delete`
> (confere o nome de novo no core), `team_start`. Arquivar e excluir param os agentes antes.
> Front: `features/teams/` — T2 com cards (cor, missão, pontos de estado, contagem, diretório,
> última atividade, ▶ Iniciar equipe, menu arquivar/excluir), T3 em 3 passos com seletor nativo de
> pasta (`tauri-plugin-dialog`) e troca de runtime por agente no passo 3. Agentes de modelo nascem
> com `autostart`. Ícone da equipe e o badge de "tarefas" do card ficam para quando houver
> seletor de ícone e o Kanban (Fase 06).

### [x] F02-09 — Painel de agente (T5)
Formulário completo de criação/edição com `react-hook-form` + `zod`, geração de handle a partir do
nome, seletor de runtime mostrando disponibilidade, atribuição de cor e seção "Avançado".
**Aceite:** validação impede handle duplicado antes de submeter; editar agente rodando avisa que
exige reinício. Depende de F02-05, F02-03.
> Feito. Front: `features/agents/agentForm.ts` (esquema zod espelhando o core, conversões
> formulário ⇄ `AgentDraft`, testado) e `AgentFormDialog.tsx` (T5: nome, endereço sugerido pelo
> `Handle::suggest` do core até ser editado à mão, papel, runtime com disponibilidade, comando do
> `custom`, modelo quando o adaptador tem `model_flag`, diretório com seletor nativo, cor e
> Avançado: entrega, autostart, reinício, autonomia, variáveis e argumentos). Core:
> `agent/ops.rs` — `update_agent` devolve `restartRequired` só quando o agente está vivo **e**
> mudou algo lido no início (nome e cor não contam). App: `agents_list`, `agent_create`,
> `agent_update`, `agent_delete` (para antes), `handle_suggest`. Para o formulário ter onde morar,
> `features/teams/TeamView.tsx`: a equipe aberta com a lista de agentes (estado, iniciar/parar,
> editar, excluir) e o terminal real do agente selecionado — o degrau até a Sala da Equipe (F03).
> Skills do agente e o preview do `BOOT.md` entram com a Fase 04.

### [ ] F02-10 — Bancadas (`git worktree` por agente)
Modos `shared` e `per-agent` por equipe, com exceção por agente. Criação, reutilização e validação
do worktree; fallback para `shared` com aviso quando o diretório não é git ou o Git é antigo.
Cópia dos arquivos ignorados declarados em `aisense.toml` e execução do `bench.setup`.
Remoção sempre por `git worktree remove`, recusando quando há mudanças não commitadas.
Ver [16 — Bancadas](../16-bancadas.md).
**Aceite:** dois agentes de uma equipe `per-agent` editam o mesmo arquivo sem se atropelar; excluir
um agente com trabalho pendente é recusado com mensagem clara. Depende de F02-06.

### [x] F02-11 — `aisense.toml` (comandos do projeto)
Parser, validação e detecção automática que **propõe** um arquivo (nunca cria sozinho) a partir de
`pnpm-lock.yaml`, `Cargo.toml`, `pyproject.toml` ou `Makefile`.
Ver [17 — Comandos do Projeto](../17-comandos-do-projeto.md).
**Aceite:** TOML inválido é reportado com caminho e linha sem derrubar o app; a proposta aparece na
UI com o conteúdo gerado para revisão.
> Feito: `aisense-core/src/project/`. `parse_project_config` aceita `test = "..."` e
> `test = { run, timeout_s }`, recusa campo desconhecido, gate que não existe em `[commands]` e
> `bench.copy` fora do repositório (`..`, caminho absoluto — seria levar segredos para a bancada).
> `load_project` devolve `found` / `invalid` (com `caminho:linha:coluna`) / `missing` com a
> proposta; a detecção reconhece pnpm, npm, yarn, Cargo, uv e Makefile e só propõe scripts que
> existem no `package.json`. Toda proposta gerada é testada como `aisense.toml` válido. Os
> utilitários de linha/coluna foram para `toml_pos.rs`, compartilhados com os adaptadores.
> App: `project_lookup` e `project_accept` (valida o conteúdo e **nunca** sobrescreve um arquivo
> existente). Front: botão "Comandos" na visão da equipe com a tabela, o erro ou a proposta
> editável. O `aisense run` em si é da Fase 05 (barramento) e a lista no `BOOT.md`, da Fase 04.

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
