# FASE 06 — Quadro Kanban

**Objetivo:** dar à equipe um estado de trabalho explícito, que agentes e humano leem e escrevem
pelas mesmas regras.

**Demonstração:** você cria 4 cartões e dá `▶ Iniciar equipe`. Sem você mandar nada, `@backend` roda
`aisense task next`, pega um cartão, move para Fazendo, implementa, comenta o commit e manda para
Revisão. `@revisor` é notificado pela automação, revisa e conclui. Você assistiu pelo quadro.

**Leitura obrigatória:** [13 — Quadro Kanban](../13-quadro-kanban.md) ·
[07 — Barramento](../07-barramento-comunicacao.md) · [04 — Modelo de Dados](../04-modelo-de-dados.md)

## Tarefas

### [ ] F06-01 — Domínio do quadro
`Board`, `Column` (com `kind` semântico e `wip_limit`), `Card`, `Label`, `ChecklistItem`,
`Dependency`, transições válidas entre colunas. Quadro criado automaticamente junto com a equipe,
com as 6 colunas padrão.
**Aceite:** criar equipe cria quadro com colunas padrão; transição inválida devolve erro tipado.
Depende de F02-01.

### [ ] F06-02 — Persistência e migração
Migração com `boards`, `columns`, `task_dependencies`, `task_comments`, `task_activity` e as colunas
novas de `tasks`, conforme [13](../13-quadro-kanban.md#esquema-adendo-a-04--modelo-de-dados).
Repositórios com consultas por coluna, responsável, label e estado.
**Aceite:** quadro com 1.000 cartões carrega em <20 ms; migração roda em banco já existente.
Depende de F06-01.

### [ ] F06-03 — `claim` atômico e limite de WIP
`UPDATE ... WHERE assignee IS NULL AND version = ?` em transação; `wip_limit` verificado na
movimentação; erros `already_claimed` e `wip_exceeded` com mensagem acionável.
**Aceite:** teste de concorrência com 8 threads disputando o mesmo cartão — exatamente uma ganha.
**Esta tarefa não é opcional: sem ela há trabalho duplicado garantido.** Depende de F06-02.

### [ ] F06-04 — Dependências, checklist, comentários e histórico
Dependências com detecção de ciclo na criação, checklist marcável, thread de comentários com autor
(agente ou humano), `task_activity` imutável registrando toda mudança com diff.
**Aceite:** ciclo `A→B→A` é recusado; concluir um cartão libera os dependentes. Depende de F06-02.

### [ ] F06-05 — Operações do quadro na CLI e no MCP
`board`, `task next|list|show|add|claim|move|update|check|comment|link|block|done|split|watch`,
todas com `--json`, e as ferramentas MCP equivalentes. Saída ASCII de `aisense board` pensada para
LLM, conforme [13](../13-quadro-kanban.md#leitura).
**Aceite:** teste de contrato garantindo que CLI e MCP produzem o mesmo efeito;
`aisense board` de uma equipe com 20 cartões cabe em menos de 60 linhas. Depende de F06-03, F05-05.

### [ ] F06-06 — Motor de automações
Gatilhos (`card_created`, `card_enters`, `card_leaves`, `card_stale`, `checklist_complete`,
`comment_added`) e ações (`assign`, `notify`, `move`, `add_label`, `unblock_dependents`,
`create_card`) — conjunto **fechado**, sem execução de comando arbitrário.
Automações padrão da equipe já configuradas na criação.
**Aceite:** cartão entrando em `review` notifica o revisor; `card_stale` dispara depois do prazo;
automação em laço é detectada e interrompida. Depende de F06-04, F05-01.

### [ ] F06-07 — Notificações pelo barramento
Toda mudança relevante vira mensagem de sistema respeitando o `delivery_mode` do destinatário,
conforme a tabela de [13](../13-quadro-kanban.md#integração-com-o-barramento).
**Aceite:** atribuir cartão a um agente em modo `hook` faz ele descobrir sozinho no fim do turno.
Depende de F06-06, F05-07.

### [ ] F06-08 — Tela do quadro (T8)
Kanban com colunas configuráveis, cartões com cor do responsável, ícones de prioridade,
dependência, checklist, comentários e bloqueio; arrastar aplicando as mesmas regras da API;
realce de 400 ms em cartão que mudou; contador de "mudou desde que você saiu".
**Aceite:** arrastar para coluna cheia mostra o mesmo erro da CLI e desfaz o movimento.
Depende de F06-05.

### [ ] F06-09 — Detalhe do cartão
Painel com corpo em Markdown, checklist, thread de comentários com avatar do agente, dependências
navegáveis, links para PR/commit/arquivo e histórico completo.
**Aceite:** comentar pela UI chega ao agente responsável como mensagem. Depende de F06-08.

### [ ] F06-10 — Editor de colunas e automações
UI para criar, renomear, reordenar e remover colunas, definir `kind` e WIP, e editar as automações
com validação (sem TOML na mão, mas com visualização do TOML gerado).
**Aceite:** remover uma coluna com cartões exige escolher para onde movê-los. Depende de F06-08.

## Critérios de saída
- [ ] Toda equipe nasce com quadro funcional
- [ ] Agente cria, lê, atualiza e conclui cartões por CLI e por MCP
- [ ] `claim` atômico comprovado sob concorrência
- [ ] WIP, dependências e bloqueio com motivo aplicados de verdade
- [ ] Automações movendo trabalho sem intervenção humana
- [ ] UI e CLI sempre consistentes (mesma fonte, mesmas regras)

## Riscos
| Risco | Mitigação |
|---|---|
| Agentes criando cartão demais e poluindo o quadro | Limite de cartões criados por agente por hora; a skill `maestro` instrui a agrupar |
| Automação em laço (A move para X, que move para Y, que move para X) | Contador de profundidade por evento, com corte e aviso na linha do tempo |
| `aisense board` ficar grande demais para o contexto | Saída resumida por padrão, com `--full` explícito; limite de linhas por coluna |
| Divergência entre regra da UI e da CLI | Regra vive só no core; UI e CLI são fachadas. Teste de contrato em F06-05 |
