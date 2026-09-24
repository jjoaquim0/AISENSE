# 09 — Telas e Fluxos

> Especificação de cada tela. Os wireframes são esquemáticos: definem hierarquia e conteúdo,
> não pixels. Estilo e tokens vêm de [08 — Design System](08-design-system.md).

## Estrutura global da janela

```
┌────────────────────────────────────────────────────────────────────────────────┐
│ ▓ AISENSE            Squad Produto ▾            ⌘K buscar       ◐ tema  ⚙      │ 40px
├──┬──────────────────┬──────────────────────────────────────────┬───────────────┤
│  │                  │                                          │               │
│▣ │  AGENTES         │                                          │  INSPETOR     │
│  │  ● @arquiteto    │            ÁREA PRINCIPAL                │               │
│▣ │  ◐ @backend      │         (4 modos de vista)               │  Skills       │
│  │  ● @frontend     │                                          │  Caixa        │
│▣ │  ○ @revisor      │                                          │  Config       │
│  │  + Novo agente   │                                          │  Logs         │
│+ │                  │                                          │               │
│  │  CANAIS          │                                          │               │
│  │  # geral         │                                          │               │
│  │  # deploys       │                                          │               │
│  ├──────────────────┼──────────────────────────────────────────┤               │
│  │  TAREFAS  3/8    │  ▸ Linha do tempo (colapsável, ⌘J)       │               │
└──┴──────────────────┴──────────────────────────────────────────┴───────────────┘
 48px      240px                  flexível                            320px
```

- **Trilho esquerdo (48px):** equipes como avatares quadrados com a cor da equipe. Badge de
  contagem quando há atividade. Arrastar reordena. `+` cria equipe.
- **Sidebar (240px, redimensionável 200–360):** agentes com estado, canais, resumo de tarefas.
- **Inspetor (320px, colapsável):** contexto do agente selecionado.
- **Linha do tempo:** gaveta inferior; pode virar painel lateral em telas largas.

---

## T1 — Primeira execução (onboarding)

Três passos, pulável, nunca mais aparece.

```
┌──────────────────────────────────────────────────────┐
│                                                      │
│              Bem-vindo ao AISENSE                    │
│     Monte equipes de IA que trabalham juntas.        │
│                                                      │
│   Encontramos no seu sistema:                        │
│   ✓ Claude Code      v2.1.0                          │
│   ✓ Codex CLI        v0.8.2                          │
│   ✗ OpenCode         não encontrado   [como instalar]│
│   ✓ Shell            /bin/zsh                        │
│                                                      │
│   Tema:  ( ) Claro   ( ) Escuro   (•) Sistema        │
│                                                      │
│                       [ Pular ]  [ Criar equipe → ]  │
└──────────────────────────────────────────────────────┘
```

A detecção de runtimes já aqui é intencional: o usuário descobre de cara o que dá para usar.

---

## T2 — Início / Equipes

Visível quando nenhuma equipe está selecionada.

```
 Suas equipes                                    [ + Nova equipe ]

 ┌───────────────────────┐  ┌───────────────────────┐  ┌────────────────────┐
 │ ▣ Squad Produto       │  │ ▣ Infra               │  │        +           │
 │ Migrar auth p/ OAuth  │  │ Monitorar produção    │  │                    │
 │                       │  │                       │  │  Nova equipe       │
 │ ●●●○  4 agentes       │  │ ●○  2 agentes         │  │                    │
 │ 3 ativos · 2 tarefas  │  │ parada               │  │  ou use um modelo  │
 │ ~/projetos/api        │  │ ~/infra               │  │                    │
 │                       │  │                       │  │                    │
 │ há 2 min          ▶   │  │ ontem             ▶   │  │                    │
 └───────────────────────┘  └───────────────────────┘  └────────────────────┘
```

Card mostra: cor/ícone, nome, missão (1 linha), pontos de estado dos agentes, contagem,
diretório, última atividade e botão **▶ Iniciar equipe** (sobe todos com `autostart`).

---

## T3 — Criar equipe (assistente, 3 passos)

**Passo 1 — Identidade:** nome, missão (textarea, com dica "isto vai no prompt de todos os agentes"),
diretório de trabalho (seletor nativo), cor e ícone.

**Passo 2 — Modelo de equipe:** atalho para não começar do zero.

| Modelo | Agentes |
|---|---|
| **Vazio** | — |
| **Dupla Dev** | `@dev` (implementador) + `@revisor` (revisor-rigoroso) |
| **Squad completo** | `@arquiteto`, `@backend`, `@frontend`, `@revisor` |
| **Pesquisa** | `@coordenador`, 3× `@pesquisador`, `@sintetizador` |
| **Operação** | `@monitor` (shell, longa duração) + `@triagem` |

Cada modelo já vem com runtime sugerido, skills e cores atribuídas. O usuário ajusta no passo 3.

**Passo 3 — Revisão:** lista dos agentes que serão criados, com runtime editável inline e avisos
(ex.: "OpenCode não está instalado — `@frontend` usará Claude Code").

---

## T4 — Sala da Equipe ⭐

A tela principal. **Quatro modos de vista**, alternáveis por `⌘G` ou pelo seletor no cabeçalho.
O modo escolhido é salvo por equipe.

### T4.1 — Grid (padrão)

Mosaico de terminais. Layouts: `1 · 2 · 3 · 4 · 6 · 9`, ou livre (arrastar/redimensionar com dnd-kit).

```
┌ Squad Produto ─ ~/projetos/api ──── [⊞ Grid][◱ Foco][⬡ Fluxo][☰ Timeline] ─ ▶ ⏸ ⟳ ┐
│                                                                                    │
│ ┌─ @arquiteto ── claude ── ● Ocioso ──── ⋮ ┐ ┌─ @backend ── codex ── ◐ Trabalhando ⋮ ┐
│ │▏                                         │ │▏                                      │
│ │▏ > Contratos revisados. O /users v2      │ │▏ Implementando o middleware de        │
│ │▏   remove legacy_id.                     │ │▏ refresh token...                     │
│ │▏                                         │ │▏ ⠋ editando src/auth/refresh.rs       │
│ │▏ ❯                                       │ │▏                                      │
│ └──────────────────────────────────────────┘ └───────────────────────────────────────┘
│ ┌─ @frontend ── opencode ── ● Ocioso ─② ─ ⋮ ┐ ┌─ @revisor ── claude ── ○ Parado ── ⋮ ┐
│ │▏                                         │ │                                       │
│ │▏ [AISENSE] Mensagem de @backend:         │ │        Agente parado                  │
│ │▏ contrato do /users v2 subiu             │ │        [ ▶ Iniciar ]                  │
│ │▏ ❯                                       │ │                                       │
│ └──────────────────────────────────────────┘ └───────────────────────────────────────┘
└────────────────────────────────────────────────────────────────────────────────────┘
```

Detalhes do painel:
- `▏` é a **borda esquerda de 3px na cor do agente** — a âncora visual principal.
- Cabeçalho: handle, runtime, estado, badge de mensagens (②), menu `⋮`
  (Reiniciar · Parar · Limpar · Duplicar · Abrir no terminal do SO · Configurar).
- O painel **focado** ganha borda de 1px em `--ring` e cabeçalho levemente mais claro.
- Painel de agente parado mostra estado vazio com ação, não um retângulo preto.
- Mensagem injetada aparece com chip `[AISENSE]` em `--fg-muted`.

### T4.2 — Foco

Um terminal grande + tira de miniaturas. Para quando você está acompanhando um agente de perto.

```
┌ ... ────────────────────────────────────────────────────────────────────┐
│ ┌─ @backend ── codex ── ◐ Trabalhando ─────────────────────────────── ⋮ ┐
│ │                                                                       │
│ │   (terminal em tamanho grande, fonte confortável)                     │
│ │                                                                       │
│ └───────────────────────────────────────────────────────────────────────┘
│ ┌─────────┐┌─────────┐┌─────────┐┌─────────┐                            │
│ │@arquit ●││@front ●②││@revis  ○││   +     │   ← clique ou ⌘1..9        │
│ └─────────┘└─────────┘└─────────┘└─────────┘                            │
└─────────────────────────────────────────────────────────────────────────┘
```

Miniaturas mostram as últimas ~4 linhas em fonte reduzida (render leve, atualizado a 2 fps — não é xterm).

### T4.3 — Fluxo (canvas) ⭐

**A vista que mostra a equipe como uma equipe.** Agentes são nós; mensagens são arestas que
animam quando trafegam. É a melhor forma de entender "quem está falando com quem" num relance.

```
┌ ... ────────────────────────────────────────────────────────────────────┐
│                                                                         │
│          ┌────────────┐                                                 │
│          │@arquiteto  │                                                 │
│          │● ocioso    │                                                 │
│          └─────┬──────┘                                                 │
│         contrato│v2                                                     │
│          ┌──────▼─────┐   "legacy_id removido" →  ┌────────────┐        │
│          │ @backend   │ ═══════════════════════►  │ @frontend  │        │
│          │◐ trabalhando│                           │● ocioso ②  │        │
│          └─────┬──────┘                           └────────────┘        │
│           ask? │                                                        │
│          ┌─────▼──────┐        ┌──────────────┐                         │
│          │ @revisor   │        │ # geral      │                         │
│          │○ parado    │        │ 12 mensagens │                         │
│          └────────────┘        └──────────────┘                         │
│                                                                         │
│  [auto-organizar] [congelar]              últimas: 5min ▾               │
└─────────────────────────────────────────────────────────────────────────┘
```

- Nó = card com cor do agente, estado, última atividade e 2 linhas de preview. Duplo clique abre o terminal.
- Aresta = uma mensagem. Espessura pela frequência, animação de partícula ao trafegar,
  tracejada para `ask` pendente (com contador regressivo do timeout), vermelha para entrega falha.
- Nós de canal aparecem como hexágonos.
- Construído com `@xyflow/react`; layout automático em camadas (quem manda acima de quem recebe;
  implementação própria, sem dagre) com posição manual persistida em `teams.layout.flow`.
- O cartão em andamento de cada agente aparece ao lado dele (aresta pontilhada); clicar abre o
  detalhe do cartão.

### T4.4 — Timeline

Todas as conversas da equipe como um chat, incluindo você.

```
┌ ... ── filtros: [todos ▾] [@backend ▾] [tipo ▾] ─── 🔍 ────────────────┐
│                                                                         │
│  ▏@arquiteto → @backend                                    14:02        │
│  ▏ O /users v2 remove legacy_id. Use external_id.                       │
│                                                                         │
│  ▏@backend → @frontend                                     14:05        │
│  ▏ contrato do /users v2 subiu no branch feat/oauth                     │
│  ▏ ✓ entregue  ✓ lida 14:06                                             │
│                                                                         │
│  ▏@backend → @revisor                            ⏳ aguardando 4:12      │
│  ▏ ? revisa o diff de HEAD~1                                            │
│  ▏ ⚠ @revisor está parado — [iniciar agente]                            │
│                                                                         │
│  ▏sistema                                                  14:09        │
│  ▏ Cadeia de resposta atingiu 12 níveis entre @backend e @frontend.     │
│  ▏ Entregas pausadas.  [continuar] [ver conversa]                       │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│ Para: [@all ▾]  escreva uma mensagem...                     ⌘↵ enviar  │
└─────────────────────────────────────────────────────────────────────────┘
```

Bolhas com a cor do remetente na borda esquerda. Recibos de entrega/leitura.
`ask` pendente com timer. Eventos de sistema em estilo distinto, com ação.
O compositor embaixo é **você participando da equipe**.

---

## T5 — Criar/editar agente (painel lateral)

```
┌─ Novo agente ─────────────────────────────────┐
│ Nome        [ Backend                       ] │
│ Endereço    [ @backend                      ] │ ← gerado do nome, editável, valida unicidade
│ Papel       [ Implementa e mantém a API...  ] │ ← vai no BOOT.md
│                                               │
│ Runtime     ( ) Claude Code    ✓ v2.1.0       │
│             (•) Codex          ✓ v0.8.2       │
│             ( ) OpenCode       ✗ não instalado│
│             ( ) Shell                         │
│             ( ) Comando customizado           │
│ Modelo      [ o3-mini                      ▾] │
│                                               │
│ Diretório   [ herdar da equipe             ▾] │
│ Cor         ● ● ● ● ● ● ● ●                   │
│                                               │
│ Skills                          [ + Adicionar]│
│  ⠿ rust-idiomatico        v2.0.1        [x]   │ ← arrastar reordena (ordem de injeção)
│  ⠿ implementador          v1.0.0        [x]   │
│                                               │
│ ▸ Avançado                                    │
│   Receber mensagens  (•) Caixa  ( ) Injetar   │
│                      ( ) Hook (recomendado)   │
│   Iniciar com a equipe        [x]             │
│   Reiniciar em caso de falha  [ on-crash   ▾] │
│   Autonomia          (•) Perguntar ( ) Confiar│
│   Variáveis de ambiente       [ editar ]      │
│   Argumentos extras           [ editar ]      │
│                                               │
│                      [ Cancelar ] [ Criar ]   │
└───────────────────────────────────────────────┘
```

O **preview do `BOOT.md`** fica acessível por um link "ver o que este agente vai receber" —
transparência total sobre o prompt injetado.

---

## T6 — Inspetor do agente

Abas no painel direito, sobre o agente selecionado:

| Aba | Conteúdo |
|---|---|
| **Visão** | Estado, tempo ativo, PID, mensagens trocadas, tarefas, últimos eventos |
| **Skills** | Ativas com toggle; aviso de "precisa reiniciar"; link para a biblioteca |
| **Caixa** | Mensagens pendentes/lidas, com botão "entregar agora" |
| **Config** | Mesmos campos do T5, edição inline |
| **Logs** | Transcrição completa da sessão, busca, exportar, sessões anteriores |

---

## T7 — Biblioteca de Skills

```
 Skills                  [ buscar ]  [ Importar ]  [ + Nova skill ]

 EMBUTIDAS                             SUAS
 ┌────────────────────┐ ┌────────────┐ ┌────────────────────┐
 │ trabalho-em-equipe │ │ coordenador    │ │ rust-idiomatico    │
 │ sempre ativa       │ │ coordena  │ │ v2.0.1 · 3 agentes │
 │ 🔒                 │ │ 1 agente   │ │ [editar] [dup]     │
 └────────────────────┘ └────────────┘ └────────────────────┘
```

O editor abre em tela cheia: Markdown à esquerda, preview à direita, frontmatter validado no topo,
e a barra inferior mostra quantos agentes usam e quais precisarão reiniciar.

---

## T8 — Quadro Kanban ⭐

Uma tela por equipe — é a quarta vista da Sala da Equipe (Grade · Foco · Mensagens · **Quadro**,
alternadas por `⌘G`). **Especificação completa em [13 — Quadro Kanban](13-quadro-kanban.md)** —
aqui fica só o resumo visual.

```
┌ Quadro — Squad Produto ─── [+ Cartão] [filtros ▾] [⚙ colunas] [automações] ───┐
│ A FAZER (4)      FAZENDO (2/4)    BLOQUEADA (1)   REVISÃO (1)   FEITA (12)     │
│ ┌────────────┐   ┌────────────┐   ┌───────────┐   ┌──────────┐  ┌───────────┐ │
│ │▏OAuth em   │   │▏Middleware │   │▏Remover   │   │▏Endpoint │  │▏Migração  │ │
│ │ /users     │   │ de refresh │   │ legacy_id │   │ /auth    │  │ de schema │ │
│ │ ▲alta  ⛓2  │   │ ✓2/4  💬3  │   │ ⚠ aguarda │   │ @backend │  │ ✓ há 1h   │ │
│ │ sem dono   │   │ @backend   │   │ decisão   │   │ →@revisor│  │           │ │
│ └────────────┘   └────────────┘   └───────────┘   └──────────┘  └───────────┘ │
└────────────────────────────────────────────────────────────────────────────────┘
```

- Colunas configuráveis com **limite de WIP aplicado** (não decorativo).
- Borda do cartão na cor do responsável — mesmo código visual dos terminais.
- Ícones: `▲` prioridade · `⛓` dependências · `✓n/m` checklist · `💬n` comentários · `⚠` bloqueio.
- Arrastar aplica exatamente as mesmas regras da CLI, incluindo WIP e automações.
- Cartão que muda pisca por 400 ms: dá para ver os agentes trabalhando ao vivo.
- Contador no topo: "3 cartões mudaram desde que você saiu".
- Detalhe do cartão: Markdown, checklist, thread de comentários com agentes, dependências
  navegáveis, links de PR/commit e histórico imutável.
- Tudo alimentado igualmente pela UI e por `aisense task ...` / ferramentas MCP.
- Na interface os ícones são os do Lucide (seta para cima, elo, caixa marcada, balão, alerta),
  com o mesmo significado dos símbolos acima.
- Arrastar para a coluna de bloqueio pede o motivo antes de mover; arrastar para coluna cheia
  mostra o erro do core (o mesmo da CLI) e o cartão volta.
- Editores de colunas e de automações (formulário, com o TOML gerado à vista); remover coluna
  com cartões pede o destino deles. Revisão: "Aprovar" e "Rejeitar" (motivo obrigatório) no
  detalhe do cartão em coluna de revisão.

---

## T9 — Configurações

| Seção | Conteúdo |
|---|---|
| **Aparência** | Tema, fonte da UI, fonte e tamanho do terminal, densidade, tema do terminal |
| **Runtimes** | Adaptadores detectados, editor de TOML, **modo calibração** de estado, instalar integração MCP |
| **Skills** | Diretório da biblioteca, import/export |
| **Barramento** | Limites anti-laço, timeout padrão do `ask`, retenção de mensagens |
| **Atalhos** | Remapeamento completo |
| **Segredos** | Chaves de API por runtime (armazenadas no keychain do SO) |
| **Avançado** | Diretório de dados, nível de log, exportar diagnóstico, resetar |

Como está feito (F08-05): `features/settings/`. Cada mudança grava na hora (números ao sair do
campo). Além das seções acima há **Notificações** (F08-07) e **Sessão** (F08-06). O modo
calibração escolhe um runtime e a tela de um agente dele que esteja rodando (relida a cada
segundo, ou um trecho colado), testa os regex a cada tecla mostrando qual linha decide e o erro
de cada regex, e "Aplicar sem reiniciar" grava o adaptador e troca as regras das sessões vivas.
Os segredos aparecem como `sk-…abcd` e nunca voltam inteiros para a interface.

---

## T10 — Paleta de comandos (⌘K)

Uma barra de busca que faz tudo. Categorias: ir para equipe/agente, criar, iniciar/parar,
enviar mensagem (`> enviar @backend ...`), aplicar skill, mudar vista, abrir configuração.
Busca difusa, resultados recentes no topo, atalho exibido à direita de cada item.

Como está feito (F07-06): `features/palette/`. O shell registra as ações globais (ir para equipe,
skills, nova equipe, tema, painéis); a tela aberta registra as dela com `usePaletteActions` — a
Sala da Equipe põe vistas, iniciar/parar/reiniciar equipe e cada agente, ir para o terminal,
novo agente, novo cartão, notas e comandos. `> enviar @agente texto` (ou `>#canal texto`) manda
pelo barramento como `@voce`. Os recentes ficam no navegador (conveniência local).

---

## Fluxos críticos (E2E da Fase 8)

| # | Fluxo | Passos |
|---|---|---|
| F1 | Do zero à equipe rodando | Onboarding → criar equipe com modelo → ▶ Iniciar → 4 terminais vivos |
| F2 | Agentes conversando | `@a` roda `aisense ask @b` → `@b` recebe → responde → `@a` destrava → tudo na timeline |
| F3 | Skill no boot | Criar skill → atribuir → reiniciar agente → conteúdo presente no `BOOT.md` |
| F4 | Recuperação de falha | Matar processo → estado `failed` → política reinicia → mensagens pendentes entregues |
| F5 | Troca de tema | Alternar claro/escuro com 9 terminais abertos → sem flash, sem reload, xterm re-tematizado |
