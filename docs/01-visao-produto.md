# 01 — Visão de Produto

## O problema

Hoje, quem usa IA para trabalhar de verdade acaba com **seis abas de terminal abertas**: uma com
Claude Code, outra com Codex, outra com OpenCode, mais duas com shells soltos. Cada uma:

- não sabe o que as outras estão fazendo;
- precisa ser reconfigurada do zero a cada início ("você é um revisor de código, siga estas regras...");
- exige que **você** seja o barramento de mensagens — copiando resposta de uma e colando na outra.

O ser humano vira o fio de cobre entre as IAs. Isso não escala.

## A solução

AISENSE é um **ambiente de trabalho para equipes de agentes de IA**. Três ideias:

### 1. Equipe como unidade de trabalho
Uma equipe é um agrupamento com **missão, diretório de trabalho e memória próprios**.
"Squad de Produto", "Time de Infra", "Pesquisa de Mercado" — cada uma com seus agentes.

### 2. Agente = terminal real + papel + skills
Cada agente é um processo de verdade em um PTY de verdade. Você escolhe o runtime
(Claude Code, Codex, OpenCode, Gemini CLI, shell puro ou um comando customizado), dá um papel
(`@revisor`, `@backend`) e anexa skills. Quando o terminal sobe, ele **já vem configurado**.

### 3. Os agentes conversam entre si
Todo agente ganha um endereço (`@backend`) e ferramentas para falar com os outros:
mandar mensagem, fazer pergunta e aguardar resposta, transmitir para a equipe, criar tarefa.
Funciona via CLI `aisense` (serve para qualquer IA em qualquer terminal) e via MCP
(para runtimes que suportam, como Claude Code e Codex).

## Para quem

| Perfil | Uso típico |
|---|---|
| **Dev solo / indie** | Dupla "implementador + revisor" trabalhando no mesmo repositório |
| **Tech lead** | Um agente por serviço, com um maestro distribuindo tarefas |
| **Pesquisador** | Agentes coletando de fontes diferentes e um sintetizador consolidando |
| **Operações/DevOps** | Terminais de longa duração monitorando, com um agente triando alertas |

## Princípios de produto

1. **Terminal real, não simulação.** É um PTY de verdade. Se funciona no seu terminal, funciona aqui —
   incluindo TUIs, cores, `htop`, editores.
2. **Local-first.** Seus dados, seu disco. Sem conta obrigatória, sem nuvem, funciona offline
   (exceto o que a IA escolhida precisar da rede).
3. **Sem lock-in de modelo.** O AISENSE não é cliente de nenhuma IA — ele **hospeda** a CLI que você já usa.
4. **Configuração é dado, não código.** Runtimes e skills são arquivos (TOML e Markdown) que o usuário
   edita. Adicionar uma IA nova não pode exigir recompilar o app.
5. **Observabilidade acima de mágica.** Toda mensagem entre agentes é visível, auditável e reproduzível
   na linha do tempo. Nada acontece escondido.
6. **O humano tem sempre o volante.** Pausar, interromper, assumir o terminal e desfazer sempre
   disponíveis. Autonomia é opt-in, por agente.

## Casos de uso guia

Estes três casos são o critério de sucesso do v1. Se eles funcionam bem, o produto funciona.

### CU-1 — Dupla implementador + revisor
`@dev` (Claude Code) implementa uma feature. Ao terminar, executa
`aisense ask @revisor "revisa o diff de HEAD~1"`. `@revisor` (Codex, com a skill "revisão rigorosa")
analisa e responde. `@dev` recebe a resposta e corrige. O humano só lê a linha do tempo.

### CU-2 — Maestro distribuindo trabalho
`@maestro` recebe do humano "migre a autenticação para OAuth". Ele quebra em tarefas no quadro,
atribui `@backend`, `@frontend` e `@docs`, acompanha status e avisa quando tudo fecha.

### CU-3 — Pesquisa paralela com síntese
Quatro agentes pesquisam fontes distintas em paralelo, cada um publicando achados em `#pesquisa`.
`@sintetizador` acompanha o canal e produz o relatório final.

## Escopo do v1

**Dentro:**
- Equipes e agentes com CRUD completo
- Runtimes: Claude Code, Codex, OpenCode, Gemini CLI, shell puro, comando customizado
- Terminais múltiplos com grid, foco, abas e atalhos
- Skills por agente com materialização e boot automático
- Barramento completo: DM, canal, broadcast, pergunta/resposta, quadro de tarefas
- Linha do tempo das conversas entre agentes
- Tema claro/escuro, paleta de comandos, atalhos de teclado
- macOS, Linux e Windows

**Fora (explicitamente):**
- Agentes remotos (SSH/container) — ver D1 em [ESTADO.md](ESTADO.md)
- Colaboração multiusuário em tempo real
- Marketplace/loja de skills
- Fine-tuning, RAG embutido ou banco vetorial
- Ser um cliente de API de LLM (chamar a API direto sem CLI)

## Nome e voz

**AISENSE** — de "AI" + "sense" (sentido, percepção): o produto dá aos agentes a percepção uns dos outros.
Tom da interface: direto, técnico, sem infantilizar. Mensagens de erro dizem o que aconteceu e o que fazer.
Zero emoji na UI de produção.
