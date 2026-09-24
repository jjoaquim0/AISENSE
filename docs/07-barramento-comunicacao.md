# 07 — Barramento de Comunicação

> O coração do AISENSE. É o que transforma "vários terminais abertos" em "uma equipe".
> Leia junto com [ADR 0004](adr/0004-protocolo-do-barramento.md) (transporte) e
> [ADR 0006](adr/0006-entrega-de-mensagens.md) (entrega).

## Endereçamento

| Forma | Exemplo | Destino |
|---|---|---|
| Agente | `@backend` | Um agente da mesma equipe |
| Canal | `#geral` | Os inscritos no canal; canal sem inscritos vai para a equipe toda |
| Equipe | `@all` | Todos os agentes ativos da equipe |
| Humano | `@voce` | Notificação na UI, sem PTY |

Canais nascem no primeiro envio (abertos) ou pela UI (T4.4, botão "Canais": tópico e inscritos).
O agente entra e sai com `aisense join #canal` / `aisense leave #canal` (entrar num canal que não
existe cria o canal só com ele) e vê a lista com `aisense channels` (MCP: `aisense_channels`).
Apagar um canal apaga as mensagens dele.

Escopo: **mensagens não atravessam equipes.** Equipes são silos por design — isso mantém o contexto
limpo e evita que um agente de "Infra" apareça no meio da conversa de "Produto". Quando for preciso
ligar duas equipes, o humano faz a ponte (ou, pós-v1, um agente com papel de *liaison*).

## Modelo de mensagem

```jsonc
{
  "v": 1,
  "id": "msg_01J8XK2P9Q...",        // ULID
  "ts": 1758556800123,               // epoch ms UTC
  "team": "tem_01J...",
  "from": "@backend",                // ou "@voce" para o humano
  "to":   ["@frontend"],             // ["#geral"] | ["@all"]
  "kind": "message",                 // message | request | response | event | system
  "reply_to": null,                  // ULID, para correlacionar request/response
  "subject": null,
  "body": "contrato do /users v2 subiu, legacy_id foi removido",
  "meta": {
    "priority": "normal",            // low | normal | high
    "timeout_s": 300,                // só em kind=request
    "attachments": [
      { "type": "file", "path": "/Users/voce/projeto/openapi.yaml" }
    ]
  }
}
```

`body` é **texto/Markdown**. Nada de estrutura obrigatória: o consumidor é um LLM, e texto claro é
o formato que ele entende melhor. Estrutura vai em `meta`.

## Transporte (IPC)

- **macOS/Linux:** Unix domain socket em `~/.aisense/run/aisense.sock`, permissão `0600`.
- **Windows:** named pipe `\\.\pipe\aisense-<user-sid>`, DACL só para o usuário.
- **Formato:** NDJSON — um objeto JSON por linha, UTF-8, `\n` como delimitador. Limite de 1 MiB por frame.
- **Autenticação:** primeiro frame obrigatoriamente `hello` com `AISENSE_TOKEN`. Token inválido,
  expirado ou ausente → o servidor responde `{"ok":false,"error":"unauthorized"}` e fecha a conexão.
- **Conexão:** efêmera para comandos simples; persistente para `wait`, `ask` e `subscribe`.

### Operações

| `op` | Parâmetros | Resposta | Bloqueia? |
|---|---|---|---|
| `hello` | `token` | identidade do agente + membros da equipe | não |
| `send` | `to[]`, `body`, `meta?` | `{ id }` | não |
| `ask` | `to`, `body`, `timeout_s?` | mensagem de resposta | **sim** (até timeout) |
| `reply` | `reply_to`, `body` | `{ id }` | não |
| `inbox` | `drain?`, `since?`, `limit?` | lista de mensagens | não |
| `wait` | `timeout_s?` | primeira mensagem que chegar | **sim** |
| `agents` | — | lista com handle, papel, runtime, estado | não |
| `status` | `state`, `note?` | `ok` | não |
| `task.add` / `.list` / `.update` / `.done` | — | tarefa(s) | não |
| `note` | `body` | `{ id }` | não |
| `subscribe` | `filter?` | stream de eventos | **sim** (persistente) |

Erros retornam `{"ok":false,"error":"<code>","message":"<humano>"}` com códigos:
`unauthorized`, `unknown_agent`, `agent_stopped`, `timeout`, `rate_limited`, `invalid_request`, `internal`.

## As três interfaces do barramento

### 1. CLI `aisense` — funciona em qualquer terminal
O caminho universal. Mesmo num `bash` puro com uma IA que você chamou manualmente, o comando está lá.
Superfície completa documentada em [06 — Sistema de Skills](06-sistema-de-skills.md#a-skill-embutida-trabalho-em-equipe).

Comportamento de saída pensado para LLM: texto limpo, sem cor quando `!isatty`, `--json` disponível
em tudo, exit code `0` sucesso / `1` erro de uso / `2` timeout / `3` destinatário indisponível.

### 2. Servidor MCP `aisense-mcp` — para runtimes que suportam
Quando o adaptador declara `capabilities.mcp = true`, o AISENSE registra o `aisense-mcp` como
servidor MCP stdio do agente. As mesmas operações viram ferramentas nativas:

| Ferramenta | Equivalente CLI |
|---|---|
| `aisense_list_agents` | `aisense agents` |
| `aisense_send_message` | `aisense send` |
| `aisense_ask_agent` | `aisense ask` |
| `aisense_read_inbox` | `aisense inbox` |
| `aisense_reply` | `aisense reply` |
| `aisense_notes` | `aisense notes ...` |
| `aisense_board` | `aisense board` |
| `aisense_next_task` | `aisense task next` |
| `aisense_list_tasks` | `aisense task list` |
| `aisense_show_task` | `aisense task show` |
| `aisense_create_task` | `aisense task add` |
| `aisense_update_task` | `aisense task claim\|move\|update\|check\|comment\|link\|block\|done\|split\|approve\|reject\|archive` (campo `action`) |
| `aisense_watch_tasks` | `aisense task watch` |

Cada ferramenta monta **o mesmo frame** do comando equivalente (teste de contrato em
`aisense-mcp/src/tools.rs`). No quadro, a resposta do servidor já traz o texto (`text`, feito por
`board::render_*` no core) — CLI e MCP mostram exatamente a mesma coisa.

Por que os dois caminhos existem: MCP é mais confiável (a IA sabe que a ferramenta existe, com schema),
mas nem todo runtime suporta. A CLI é o denominador comum. **Ambos chamam exatamente o mesmo core** —
sem duplicação de lógica.

### 3. UI — o humano é um participante
Você manda mensagem para qualquer agente pela linha do tempo, e as suas mensagens entram no mesmo
fluxo (`from: "@voce"`). Um agente pode te perguntar algo: vira notificação com campo de resposta.

## Entrega: os três modos

Configurável **por agente** (`agents.delivery_mode`):

### `pull` (padrão — seguro)
A mensagem fica na caixa de entrada. O agente lê quando rodar `aisense inbox`.
Na UI, o badge do agente acende com a contagem.
- ✅ Nunca corrompe o que o agente está fazendo. Zero risco.
- ❌ Depende de o agente lembrar de checar (a skill `trabalho-em-equipe` instrui isso).

### `push` (proativo)
Quando o detector de estado diz `idle`, o AISENSE escreve a mensagem no stdin do PTY:

```
[AISENSE] Mensagem de @frontend: o contrato do /users v2 quebrou o build do cliente. ↵
```

Regras rígidas de segurança:
1. Só injeta com estado `idle` **e** confiança alta.
2. **Nunca** injeta com estado `awaiting_input` (a IA está pedindo confirmação ao humano).
3. Fila FIFO por agente; se chegarem 5 mensagens durante um `busy`, são agrupadas em uma só injeção.
4. Sanitização obrigatória: remove todo controle C0/C1 (inclui `\x1b`, `\x07`, `\r`) e o DEL,
   junta o corpo numa linha só (`\n` vira ` / `) e limita a `inject.max_chars`; acima disso
   injeta só o aviso "N mensagem(ns) longa(s) de @x — leia com: aisense inbox" (o corpo inteiro
   continua na caixa; nada é gravado em arquivo).
5. Throttle: no máximo 1 injeção por agente a cada 3 s.
6. A UI mostra um chip **"mensagem injetada"** na linha do terminal — nada é invisível.

### `hook` (o melhor, quando disponível)
Para runtimes com hooks (Claude Code), o AISENSE instala um hook de fim de turno que roda
`aisense inbox --drain --if-any --hook-json`. Com mensagem nova, a CLI responde
`{"decision":"block","reason":"<as mensagens>"}` — o formato do hook `Stop` que faz o agente
continuar em vez de parar, já com as mensagens em mãos; sem mensagem, não imprime nada e ele para
normalmente. Fora de um terminal do AISENSE (o mesmo projeto aberto à mão), o comando sai calado
com exit 0. Resultado: o agente checa a caixa **sozinho**, no momento certo, sem injeção e sem
depender da heurística de estado.

```jsonc
// <workdir>/.claude/settings.json (mesclado, não sobrescrito; instalado no start do agente)
{
  "hooks": {
    "Stop": [{ "hooks": [{ "type": "command", "command": "aisense inbox --drain --if-any --hook-json" }] }]
  }
}
```

**Recomendação:** `hook` quando o runtime suportar, `pull` no resto, `push` só para agentes de
longa duração que você quer reativos (ex.: um monitor).

## Pergunta e resposta (`ask`)

```
@backend                 core/bus                 @frontend
   │ ask(@frontend,...)      │                        │
   ├────────────────────────►│ persiste request       │
   │  (conexão fica aberta)  ├───────────────────────►│ entrega (pull/push/hook)
   │                         │                        │ trabalha...
   │                         │◄───────────────────────┤ reply(msg_id, "...")
   │◄────────────────────────┤ correlaciona reply_to  │
   │ imprime resposta, exit 0│                        │
```

- Timeout padrão **300 s**, máximo 1800 s. No timeout: exit code `2` com mensagem
  "sem resposta de @frontend em 300s — siga sem ela ou tente de novo".
- Se o destinatário estiver parado: erro imediato `agent_stopped` (não espera o timeout).
  A resposta sugere `aisense send` para deixar recado.
- **Detecção de deadlock:** se A está bloqueado esperando B e B pede um `ask` para A, o core recusa
  o segundo com `would_deadlock` e explica. Implementado como detecção de ciclo no grafo de `ask` pendentes.

## Proteções contra laço infinito

Dois agentes podem entrar num ping-pong eterno e queimar tokens. Guardas obrigatórias:

| Guarda | Padrão | Comportamento ao estourar |
|---|---|---|
| Taxa por agente | 30 msg/min | `rate_limited`; CLI orienta a agrupar |
| Profundidade de cadeia de reply | 12 | Injeta aviso de sistema: "cadeia longa, resuma e decida" |
| Mensagens idênticas repetidas | 3 | Bloqueia a 4ª e notifica o humano |
| Orçamento da equipe | 500 msg/h | Pausa entregas e abre diálogo: [Continuar] [Pausar equipe] |

Todos configuráveis em Configurações → Equipe. Todo bloqueio aparece na linha do tempo — nunca silencioso.

## Eventos para a UI

O core emite via Tauri (a UI nunca faz polling):

| Evento | Payload |
|---|---|
| `bus:message` | mensagem completa |
| `bus:delivery` | `{ messageId, agentId, state }` |
| `bus:blocked` | `{ reason, agentId, detail }` |
| `agent:state` | `{ agentId, state, confidence }` |
| `pty:data` | `{ agentId, chunk }` (só de painel visível) |
| `pty:exit` | `{ agentId, code }` |
| `task:changed` | tarefa completa |

## Ordem de implementação sugerida (Fase 5)

1. `core::bus` com repositório em memória + testes de roteamento (sem I/O)
2. Persistência em `messages`/`deliveries`
3. Servidor `aisense-ipc` com `hello`/`send`/`inbox`
4. CLI `aisense` cobrindo essas três ops → **já dá para dois agentes trocarem recado**
5. `ask`/`reply` com correlação e timeout
6. Modo `push` + fila + sanitização
7. Modo `hook` para Claude Code
8. `aisense-mcp`
9. Guardas anti-laço
10. Linha do tempo na UI
