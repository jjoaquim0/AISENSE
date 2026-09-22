# FASE 05 — Barramento de Comunicação

> **Esta é a fase que define o produto.** Tudo antes é infraestrutura para chegar aqui.
> Se precisar cortar escopo em qualquer lugar, não corte aqui.

**Objetivo:** os agentes conversam entre si.

**Demonstração:** `@backend` roda `aisense ask @revisor "revisa o diff"`, `@revisor` recebe,
trabalha, responde, `@backend` destrava com a resposta — e tudo aparece na linha do tempo.

**Leitura obrigatória:** [07 — Barramento](../07-barramento-comunicacao.md) ·
[ADR 0004](../adr/0004-protocolo-do-barramento.md) · [ADR 0006](../adr/0006-entrega-de-mensagens.md) ·
[11 — Segurança](../11-seguranca.md)

## Tarefas

### [ ] F05-01 — Núcleo do barramento em memória
`core::bus`: modelo de `Message`, resolução de endereço (`@agente`, `#canal`, `@all`, `@voce`),
roteamento, criação de `deliveries` por destinatário. Sem I/O — repositório como trait.
**Aceite:** testes de roteamento cobrindo DM, canal, broadcast, destinatário inexistente e
destinatário parado. Não depende de nada além do domínio.

### [ ] F05-02 — Persistência de mensagens
Repositórios de `messages`, `deliveries` e `channels`; consultas de caixa de entrada e de linha do
tempo com paginação por cursor; rotina de retenção.
**Aceite:** inserir 100.000 mensagens e consultar a timeline de uma equipe em <20 ms.
Depende de F05-01, F02-02.

### [ ] F05-03 — Servidor IPC
`aisense-ipc`: socket UDS / named pipe com permissão restrita, framing NDJSON com limite de 1 MiB,
`hello` com validação de token, despacho de operações para o core, encerramento limpo.
**Aceite:** conexão sem token é recusada e fechada; frame maior que o limite é rejeitado sem
derrubar o servidor. Depende de F05-01.

### [ ] F05-04 — Emissão de tokens de sessão
Gerar `AISENSE_TOKEN` por sessão de agente, gravar em `agent_tokens` com expiração, injetar no
ambiente do PTY, revogar ao encerrar a sessão.
**Aceite:** token de sessão encerrada é rejeitado imediatamente. Depende de F05-03, F02-06.

### [ ] F05-05 — CLI `aisense`
Binário com `whoami`, `agents`, `send`, `inbox`, `note`, `status`; `--json` em tudo; saída sem cor
quando não é TTY; exit codes de [07](../07-barramento-comunicacao.md#as-três-interfaces-do-barramento).
Empacotado como sidecar e injetado no PATH dos PTYs.
**Aceite:** dentro de um agente `shell`, `aisense send @outro "oi"` funciona e a mensagem aparece na
UI em <50 ms. **Este é o marco que prova o produto.** Depende de F05-04.

### [ ] F05-06 — Pergunta e resposta (`ask`/`reply`)
Conexão persistente bloqueante com correlação por `reply_to`, timeout configurável, erro imediato
quando o destinatário está parado, detecção de deadlock por ciclo no grafo de `ask` pendentes.
**Aceite:** `ask` mútuo entre dois agentes é recusado com `would_deadlock`; timeout retorna exit 2
com mensagem acionável. Depende de F05-05.

### [ ] F05-07 — Entrega em modo `push`
Fila por agente, gatilho por `idle` com confiança alta, bloqueio absoluto em `awaiting_input`,
throttle de 3 s, agrupamento de mensagens acumuladas, sanitização obrigatória.
**Aceite:** o teste de sanitização com corpus malicioso passa (incluindo `\r`, ESC, OSC);
nenhuma injeção ocorre com o agente em `awaiting_input`. Depende de F05-05, F03-01.

### [ ] F05-08 — Entrega em modo `hook`
Mesclar (nunca sobrescrever) o hook de fim de turno em `.claude/settings.json` para runtimes com
`capabilities.hooks = true`, executando `aisense inbox --drain --if-any`.
**Aceite:** um agente Claude Code checa a caixa sozinho ao fim de cada turno; configuração
pré-existente do usuário é preservada. Depende de F05-05.

### [ ] F05-09 — Servidor MCP `aisense-mcp`
Binário MCP stdio expondo `aisense_list_agents`, `aisense_send_message`, `aisense_ask_agent`,
`aisense_read_inbox`, `aisense_create_task`, `aisense_update_task`. Instalação automática na
configuração do runtime quando `capabilities.mcp = true`.
**Aceite:** teste de contrato garantindo que CLI e MCP produzem exatamente o mesmo efeito no core.
Depende de F05-06.

### [ ] F05-10 — Guardas anti-laço
Limite de taxa por agente, profundidade de cadeia de reply, detecção de mensagens repetidas,
orçamento por hora da equipe. Todo bloqueio vira mensagem de sistema visível com ação.
**Aceite:** dois agentes em ping-pong são interrompidos dentro do limite configurado e o humano é
avisado com opção de continuar. Depende de F05-01.

### [ ] F05-11 — Vista Timeline (T4.4)
Lista virtualizada de mensagens com cor do remetente, recibos de entrega/leitura, `ask` pendente com
contagem regressiva, eventos de sistema com ação, filtros e compositor do humano (`@voce`).
**Aceite:** 10.000 mensagens rolam a 60 fps; mensagem nova entra sem saltar o scroll quando o
usuário está lendo histórico. Depende de F05-02.

## Critérios de saída
- [ ] Dois agentes trocam mensagens em qualquer combinação de runtimes, inclusive `shell` puro
- [ ] `ask`/`reply` funciona com timeout e sem deadlock
- [ ] Os três modos de entrega funcionam e são testados
- [ ] Nenhuma injeção insegura é possível (corpus de sanitização verde)
- [ ] Guardas anti-laço ativas por padrão
- [ ] Toda comunicação é visível na linha do tempo

## Riscos
| Risco | Mitigação |
|---|---|
| Injeção no momento errado causar dano real | Padrão é `pull`; `push` exige todas as condições de [ADR 0006](../adr/0006-entrega-de-mensagens.md); `awaiting_input` bloqueia sempre |
| Agentes entrarem em laço e queimarem tokens | F05-10 é **obrigatória**, não opcional, e ligada por padrão |
| A IA não lembrar de checar a caixa | Skill `trabalho-em-equipe` instrui explicitamente; modo `hook` onde houver suporte; badge visível na UI |
| Named pipe no Windows com permissão frouxa | DACL explícita para o SID do usuário; teste específico no Windows |
