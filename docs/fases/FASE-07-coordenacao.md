# FASE 07 — Coordenação

**Objetivo:** sair de "agentes que conversam e têm um quadro" para "uma equipe que se organiza".

**Demonstração:** dizer ao `@coordenador` "migre a autenticação para OAuth"; ele quebra o objetivo em
cartões, atribui a `@backend`, `@frontend` e `@docs`, acompanha o quadro e avisa quando tudo fecha.
A vista Fluxo mostra as mensagens trafegando ao vivo.

**Leitura obrigatória:** [13 — Quadro Kanban](../13-quadro-kanban.md) ·
[09 — Telas](../09-telas-e-fluxos.md#t43--fluxo-canvas-) ·
[11 — Segurança](../11-seguranca.md#comandos-do-barramento-que-exigem-confirmação-humana)

## Tarefas

### [ ] F07-01 — Skill `coordenador` completa
Ensina a quebrar objetivo em cartões com dependências, atribuir por papel, acompanhar o quadro,
consolidar o resultado — e, principalmente, **quando não delegar** (trabalho trivial é dele mesmo)
e como escalar para o humano quando trava.
**Aceite:** com a equipe "Squad completo", o coordenador executa o caso CU-2 de
[01 — Visão](../01-visao-produto.md#cu-2--coordenador-distribuindo-trabalho) de ponta a ponta.
Depende de F06-05.

### [ ] F07-02 — Propostas do coordenador
Ações estruturais (criar agente, mudar autonomia, editar skill, alterar colunas) viram **proposta**
na UI com [Aceitar]/[Recusar] — nunca execução direta.
**Aceite:** agente tentando criar outro agente recebe erro do barramento e a proposta aparece na UI.
Depende de F06-05.

### [ ] F07-03 — Vista Fluxo (T4.3)
Canvas com `@xyflow/react`: nós de agente e de canal, arestas animadas por mensagem, tracejado com
contagem regressiva para `ask` pendente, aresta vermelha para falha, layout automático (dagre) com
posição manual persistida, filtro por janela de tempo.
**Aceite:** com 6 agentes trocando mensagens, o canvas mantém ≥50 fps. Depende de F05-11.

### [ ] F07-04 — Cartões no canvas
Nó de agente mostra o cartão em que ele está trabalhando; clicar abre o detalhe.
Aresta pontilhada liga agente ao cartão.
**Aceite:** mover um cartão no quadro atualiza o canvas em <300 ms. Depende de F07-03, F06-08.

### [x] F07-05 — Canais
CRUD de canais, inscrição de agentes, `aisense send #canal`, filtro na timeline, nó no canvas.
**Aceite:** o caso CU-3 (pesquisa paralela com síntese) funciona via canal. Depende de F05-02.

> Feito: `channel_members` (migração 0006) e regra de roteamento: com inscritos, só eles
> recebem; sem inscritos, a equipe toda (como antes). `BusService::{channels, save_channel,
> delete_channel, subscribe_channel}`; CLI `aisense channels|join|leave`, MCP `aisense_channels`,
> comandos `channels_list|channel_save|channel_delete` e o diálogo "Canais" na linha do tempo
> (tópico, inscritos, apagar); canais entram no filtro De/para e nos destinos do compositor.
> Teste do CU-3: quatro pesquisadores publicam em `#pesquisa`, o sintetizador recebe os 4 e quem
> está fora não recebe nada. O nó de canal no canvas entra com a F07-03.

### [ ] F07-06 — Paleta de comandos (T10)
`cmdk` com navegação, criação, controle de agentes, envio de mensagem, operações do quadro,
troca de vista e configurações. Busca difusa e recentes no topo.
**Aceite:** toda ação principal é alcançável por `⌘K` sem mouse. Depende de F03-08.

## Critérios de saída
- [ ] Um coordenador coordena 4 agentes até concluir um objetivo, usando o quadro como instrumento
- [ ] Canvas mostra agentes, mensagens e cartões em tempo real
- [ ] Nenhum agente executa ação estrutural sem aprovação humana

## Riscos
| Risco | Mitigação |
|---|---|
| Coordenador delegar tudo e virar gargalo | A skill instrui explicitamente a executar o trivial e a usar o quadro em vez de mensagens |
| Canvas pesado com muitas arestas | Agregar arestas por par de agentes com espessura por volume; limitar a janela de tempo |
| Coordenador e quadro darem instruções conflitantes | O quadro é a fonte da verdade; a skill diz isso em primeira linha |
