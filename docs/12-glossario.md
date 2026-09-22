# 12 — Glossário

Vocabulário canônico. Use exatamente estes termos em código, UI e documentação.
Sinônimo é fonte de confusão para agentes que entram no meio do projeto.

| Termo (pt-BR) | Termo no código | Definição |
|---|---|---|
| **Equipe** | `Team` | Agrupamento de agentes com missão, diretório e canais próprios. Unidade de isolamento |
| **Agente** | `Agent` | Um membro da equipe: um processo em PTY com endereço, papel, runtime e skills |
| **Endereço** | `handle` | Identificador do agente no barramento, sem `@`. Ex.: `backend` (exibido `@backend`) |
| **Papel** | `role` | Descrição do que o agente faz. Vai no prompt de bootstrap |
| **Runtime** | `runtime` | A CLI que o agente executa: Claude Code, Codex, OpenCode, shell... |
| **Adaptador** | `Adapter` | Arquivo TOML que ensina o AISENSE a operar um runtime |
| **Skill** | `Skill` | Pacote Markdown de instruções carregado no boot do agente |
| **Bootstrap** | `BOOT.md` | Documento gerado que dá identidade e instruções ao agente ao nascer |
| **Materializar** | `materialize` | Copiar as skills resolvidas para o diretório de trabalho do agente |
| **Barramento** | `Bus` | Subsistema que roteia mensagens entre agentes |
| **Mensagem** | `Message` | Unidade de comunicação. Tipos: `message`, `request`, `response`, `event`, `system` |
| **Pergunta** | `ask` / `request` | Mensagem que bloqueia o remetente até a resposta ou o timeout |
| **Caixa de entrada** | `inbox` | Mensagens pendentes de um agente |
| **Entrega** | `Delivery` | Estado de uma mensagem em relação a **um** destinatário |
| **Modo de entrega** | `delivery_mode` | `pull` (agente busca) · `push` (injeta no PTY) · `hook` (runtime checa sozinho) |
| **Injeção** | `injection` | Escrever no stdin do PTY como se o usuário tivesse digitado |
| **Canal** | `Channel` | Tópico da equipe para mensagens de muitos-para-muitos |
| **Transmissão** | `broadcast` | Mensagem para todos os agentes ativos da equipe |
| **Painel** | `Pane` | Retângulo na tela que hospeda o terminal de um agente |
| **Sala da Equipe** | `TeamRoom` | Tela principal, com os 4 modos de vista |
| **Vista** | `View` | Grid, Foco, Fluxo ou Timeline |
| **Linha do tempo** | `Timeline` | Vista cronológica de todas as mensagens da equipe |
| **Coordenador** | `coordenador` | Papel (via skill) de agente coordenador. **Não** é um tipo especial de agente |
| **Quadro** | `Board` | Kanban da equipe. Uma equipe, um quadro. É a memória compartilhada |
| **Coluna** | `Column` | Etapa do quadro, com tipo semântico e limite de WIP |
| **Cartão** | `Card` / `Task` | Unidade de trabalho no quadro |
| **Pegar** | `claim` | Um agente assumir um cartão sem dono. Operação atômica |
| **Limite de WIP** | `wip_limit` | Máximo de cartões simultâneos numa coluna. Aplicado, não sugerido |
| **Automação** | `Automation` | Regra declarativa do quadro (gatilho → ação), de conjunto fechado |
| **Bancada** | `Workbench` | (proposto) `git worktree` isolado por agente — ver doc 14 |
| **Formação** | `Formation` | (proposto) Modelo exportável de equipe inteira — ver doc 14 |
| **Agendamento** | `Schedule` | (proposto) Disparo de trabalho por horário ou intervalo — ver doc 14 |
| **Prévia ao vivo** | `LivePreview` | (proposto) Painel com o app em execução — ver doc 14 |
| **Repasse** | `hand-off` | (proposto) Regra que manda o trabalho de um agente ao próximo — ver doc 14 |
| **Nota da equipe** | `Note` | (proposto) Markdown compartilhado como memória — ver doc 14 |
| **Estado do agente** | `AgentState` | `stopped · starting · idle · busy · awaiting_input · failed` |
| **Detector de estado** | `StateDetector` | Heurística que infere o estado pela saída do PTY |
| **Ring buffer** | `RingBuffer` | Últimas N linhas de saída mantidas em RAM no Rust |
| **Sidecar** | `sidecar` | Binário auxiliar empacotado com o app (`aisense`, `aisense-mcp`) |

## Termos proibidos (e o que usar no lugar)

| Não use | Use |
|---|---|
| "chat", "conversa" para o painel | **terminal** ou **painel** |
| "bot", "assistente" | **agente** |
| "workspace" para equipe | **equipe** |
| "prompt" para o BOOT.md | **bootstrap** |
| "plugin" para skill | **skill** |
| "sessão" para agente | **agente** (sessão é a execução de PTY) |
| "worker", "thread" para agente | **agente** |
| "partitura", "andar", "portal", "rotina", "ombro" | Vocabulário de outra ferramenta. Use **formação**, **bancada**, **prévia ao vivo**, **agendamento**, **resumo de ausência** |
| "maestro", "regente" e a metáfora musical em geral | **coordenador**. A identidade do AISENSE é de equipe de engenharia, não de orquestra |
