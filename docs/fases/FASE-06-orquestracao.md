# FASE 06 — Orquestração

**Objetivo:** sair de "agentes que conversam" para "uma equipe que se organiza".

**Demonstração:** dizer ao `@maestro` "migre a autenticação para OAuth"; ele quebra em tarefas,
atribui a `@backend`, `@frontend` e `@docs`, acompanha o progresso e avisa quando fecha.
A vista Fluxo mostra as mensagens trafegando ao vivo.

**Leitura obrigatória:** [07 — Barramento](../07-barramento-comunicacao.md) ·
[09 — Telas](../09-telas-e-fluxos.md#t43--fluxo-canvas-) · [11 — Segurança](../11-seguranca.md#comandos-do-barramento-que-exigem-confirmação-humana)

## Tarefas

### [ ] F06-01 — Domínio de tarefas
`Task` com status, responsável, criador, subtarefas; transições válidas; repositório.
**Aceite:** transições inválidas são rejeitadas com erro tipado. Depende de F05-01.

### [ ] F06-02 — Operações de tarefa no barramento
`task.add`, `task.list`, `task.update`, `task.done` na CLI e no MCP; mensagem de sistema para o
responsável quando uma tarefa é atribuída ou muda de status.
**Aceite:** `aisense task add "x" --assign @backend` cria a tarefa e notifica o agente.
Depende de F06-01, F05-05.

### [ ] F06-03 — Quadro de tarefas (T8)
Kanban com 5 colunas, arrastar para mudar status, cards com responsável e origem, filtros.
Arrastar emite mensagem de sistema ao responsável.
**Aceite:** mover um card de "A fazer" para "Fazendo" notifica o agente responsável.
Depende de F06-02.

### [ ] F06-04 — Skill `maestro` completa
Escrever a skill que ensina a quebrar objetivo em tarefas, atribuir por papel, acompanhar e
consolidar — incluindo quando **não** delegar e como pedir ajuda ao humano.
**Aceite:** com a equipe "Squad completo", o maestro executa o caso de uso CU-2 de
[01 — Visão](../01-visao-produto.md#cu-2--maestro-distribuindo-trabalho) de ponta a ponta.
Depende de F06-02.

### [ ] F06-05 — Propostas do maestro
Ações que exigem humano (criar agente, mudar autonomia, editar skill) viram **propostas** na UI com
[Aceitar]/[Recusar], nunca execução direta.
**Aceite:** um agente tentando criar outro agente recebe erro do barramento e a proposta aparece
na UI. Depende de F06-02.

### [ ] F06-06 — Vista Fluxo (T4.3)
Canvas com `@xyflow/react`: nós de agente e de canal, arestas animadas por mensagem, tracejado com
contagem regressiva para `ask` pendente, aresta vermelha para falha, layout automático com posição
manual persistida, filtro por janela de tempo.
**Aceite:** com 6 agentes trocando mensagens, o canvas mantém ≥50 fps. Depende de F05-11.

### [ ] F06-07 — Canais
CRUD de canais, inscrição de agentes, `aisense send #canal`, filtro na timeline, nó no canvas.
**Aceite:** o caso de uso CU-3 (pesquisa paralela com síntese) funciona via canal.
Depende de F05-02.

### [ ] F06-08 — Paleta de comandos (T10)
`cmdk` com navegação, criação, controle de agentes, envio de mensagem, troca de vista e
configurações. Busca difusa e recentes no topo.
**Aceite:** toda ação principal do app é alcançável por `⌘K` sem usar o mouse. Depende de F03-08.

## Critérios de saída
- [ ] Um maestro coordena uma equipe de 4 agentes até concluir um objetivo
- [ ] Quadro de tarefas sincronizado entre UI e CLI
- [ ] Canvas mostra o fluxo de mensagens em tempo real
- [ ] Nenhum agente executa ação estrutural sem aprovação humana

## Riscos
| Risco | Mitigação |
|---|---|
| Maestro delegar demais e virar gargalo | A skill instrui explicitamente a fazer o trabalho trivial ele mesmo |
| Canvas ficar pesado com muitas arestas | Agregar arestas por par de agentes, com espessura por volume; limitar a janela de tempo |
| Tarefas divergirem entre CLI e UI | Fonte única no core; UI e CLI são fachadas do mesmo repositório |
