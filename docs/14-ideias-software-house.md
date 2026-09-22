# 14 — Ideias: o AISENSE como software house

> Backlog priorizado de recursos que transformam "equipe de agentes" em "software house que opera
> sozinha". Parte vem da análise do **Maestri** (referência declarada do projeto), parte é prática
> conhecida de time de engenharia.
>
> **Nada aqui entra no escopo sem sair desta lista e virar fase.** Este documento existe para
> registrar a ideia com a análise feita — não para ser implementado por conta própria.

## O que aprendemos com o Maestri

O [Maestri](https://www.themaestri.app/en) é um app nativo de macOS/Windows que organiza agentes de
código (Claude Code, Codex, Gemini, OpenCode) num canvas infinito, onde terminais são nós que você
conecta com uma linha e passam a se prompter mutuamente via orquestração de PTY.

Cinco ideias dele que valem adotar, com o nome que damos a cada uma aqui:

| No Maestri | No AISENSE | Por que vale | Prioridade |
|---|---|---|---|
| Notas markdown como fonte da verdade | **Notas da equipe** | Memória que sobrevive ao reinício do agente | 🔴 v1 |
| Andares (cópias isoladas do repo) | **Bancadas** (git worktree por agente) | Impede agentes se atropelarem no mesmo checkout | 🔴 v1 |
| Conectar agentes com uma linha | **Conexões no canvas** | A topologia da equipe vira configuração visual | 🟡 v1.1 |
| Partituras (+257 templates) | **Formações** | Montar um squad inteiro em um clique, e compartilhar | 🟡 v1.1 |
| Rotinas (tarefas agendadas) | **Rotinas** | Monitorar CI, triar, fazer deploy sem você pedir | 🟡 v1.1 |
| Ombro (resumo do que passou) | **Resumo de ausência** | Voltar depois de 2h sem ler 9 terminais | 🟡 v1.1 |
| Portais (preview ao vivo) | **Portais** | O agente verifica o que construiu, não só compila | 🟢 pós-v1 |

Onde nos diferenciamos de propósito: o Maestri conecta terminais para um agente **digitar no
terminal do outro**. No AISENSE a conversa passa por um **barramento com mensagens persistidas,
recibos e auditoria** ([ADR 0004](adr/0004-protocolo-do-barramento.md)) — e a injeção em stdin é só
um dos três modos de entrega, com todas as travas de [ADR 0006](adr/0006-entrega-de-mensagens.md).
Isso nos dá linha do tempo, `ask`/`reply` com correlação e proteção anti-laço, que digitar no
terminal alheio não dá.

---

## 🔴 Recomendado para o v1

### 1. Notas da equipe (memória compartilhada)
Arquivos Markdown por equipe que qualquer agente lê e escreve, versionados no diretório de trabalho.
É onde ficam decisões, contratos de API, convenções — o que hoje se perde quando um terminal reinicia.

```bash
aisense notes list
aisense notes read contratos-api
aisense notes write contratos-api --append "POST /auth/token devolve refresh_token desde a v2"
aisense notes search "legacy_id"
```

Guardadas em `<workdir>/.aisense/notes/*.md`, editáveis na UI com preview, e **entram no `BOOT.md`
como índice** (títulos e resumo, não o conteúdo inteiro) para o agente saber que elas existem.
Custo: baixo. Valor: alto — resolve o esquecimento entre sessões, que é o maior problema prático.

### 2. Bancadas (git worktree por agente)
Dois agentes editando o mesmo checkout é conflito garantido. Uma bancada é um `git worktree` com
branch próprio (`aisense/backend`), criado no start do agente e removido no fim.

- A equipe escolhe: **checkout compartilhado** (padrão, simples) ou **bancada por agente**.
- O quadro mostra em qual branch cada cartão está sendo feito.
- Ao concluir, o agente abre PR ou o maestro faz o merge.

Sem isso, "equipe de agentes" só funciona com um agente escrevendo por vez. **Esta é, na minha
avaliação, a funcionalidade de maior impacto desta lista inteira.**

### 3. Comandos do projeto declarados uma vez
Um `aisense.toml` no repositório dizendo como rodar, testar e lintar:

```toml
[project]
install = "pnpm install"
dev     = "pnpm dev"
test    = "pnpm test"
lint    = "pnpm lint"
build   = "pnpm build"
```

Aí `aisense run test` funciona igual para todo agente, entra no `BOOT.md`, e o quadro pode exigir
teste verde antes de mover para Revisão. Elimina cada agente inventando o comando do projeto.

### 4. Gate de revisão no quadro
Um cartão só entra em `Feita` se um agente **diferente** do responsável marcar aprovação.
Regra por coluna, configurável, desligável. É a prática de software house que mais reduz retrabalho,
e no nosso caso é barata: o quadro já sabe quem fez e quem revisou.

---

## 🟡 Candidatos a v1.1

### 5. Formações (partituras)
Exportar uma equipe inteira — agentes, papéis, skills, colunas do quadro, automações, conexões —
como um `.aisense-formacao.json` que outra pessoa importa e roda. É o que faz a ferramenta virar
comunidade: "baixe a formação Squad React + API Rust".

### 6. Biblioteca de papéis
Hoje `role` é um campo de texto no agente. Vira biblioteca reutilizável, separada das skills:
**papel = quem o agente é** (Arquiteto, Revisor, QA); **skill = como ele faz algo específico**.
Um papel referencia skills. Facilita montar equipes sem reescrever instrução.

### 7. Conexões e handoffs no canvas
A vista Fluxo deixa de ser só observação e vira configuração: você desenha a linha
`@dev → @revisor` e define o handoff ("ao concluir um cartão, mande para revisão do @revisor").
Opcionalmente restringe quem fala com quem — topologia em estrela (tudo passa pelo maestro) ou malha.

### 8. Rotinas (tarefas agendadas)
Cron por equipe: "todo dia 9h, `@triagem` lê as issues novas e cria cartões";
"a cada 15 min, `@monitor` checa o CI e abre cartão se quebrou".
Reaproveita todo o barramento — é só um disparador.

### 9. Resumo de ausência
Ao voltar depois de N minutos: o que cada agente fez, o que mudou no quadro, o que está bloqueado,
o que precisa de você. Gerado a partir da linha do tempo e do histórico do quadro — **não precisa
de modelo externo**, é sumarização de dados estruturados que já temos.

### 10. Métricas da equipe
Throughput (cartões/dia), lead time, cycle time por coluna, taxa de retrabalho (cartões devolvidos
da revisão), tempo médio bloqueado, cartões por agente. Vira uma aba do quadro.
Serve para responder "essa equipe está funcionando?" com dado, não com sensação.

### 11. Integração com Git e PR
O agente abre PR de verdade; o cartão mostra status do PR e do CI; PR mergeado move o cartão para
`Feita` automaticamente. Detectar o remoto e usar `gh`/`glab` quando existirem.

### 12. Orçamento e custo
Teto de gasto por equipe e por período, com pausa automática ao atingir. Custo por cartão,
quando o runtime reportar uso. (Ver D2 em [ESTADO.md](ESTADO.md) — depende do que cada CLI expõe.)

---

## 🟢 Pós-v1

| Ideia | Descrição |
|---|---|
| **Portais** | Painel com webview do app em desenvolvimento; o agente tira screenshot e verifica o próprio trabalho |
| **Revisão adversarial** | Executor e revisor obrigatoriamente de fornecedores diferentes (Claude implementa, Codex revisa) — diversidade reduz ponto cego comum |
| **Sprints e ciclos** | Agrupar cartões em ciclos com meta e data, com retrospectiva automática |
| **Templates de cartão** | Formulários por tipo (bug, feature, chore) com campos e checklist obrigatórios |
| **Importar de Linear/Jira/GitHub Issues** | Puxar o backlog que já existe em vez de recadastrar |
| **Replay de sessão** | Reproduzir a linha do tempo de um período como um vídeo, para entender o que deu errado |
| **Agentes remotos** | Rodar agentes em SSH ou container (ver D1 em [ESTADO.md](ESTADO.md)) |
| **Modo daemon** | Usar o AISENSE sem GUI, do próprio terminal (ver D4) |
| **Marketplace de formações e skills** | Distribuição comunitária (ver D3) |
| **Base de conhecimento com busca** | Notas + decisões + histórico do quadro com busca semântica local |

---

## O que eu recomendaria NÃO fazer

Registrado para não voltar à mesa a cada ciclo:

| Ideia tentadora | Por que não |
|---|---|
| Deixar o agente criar/configurar outros agentes livremente | Agente que cria agente é recursão sem caso base. Fica como **proposta** com aprovação humana ([11 — Segurança](11-seguranca.md)) |
| Chamar a API dos modelos direto, sem CLI | Vira mais um cliente de LLM e perde o motivo de existir: hospedar a ferramenta que você já usa |
| RAG/banco vetorial embutido | Cada runtime já faz sua busca no repositório. Duplicaríamos mal o que eles fazem bem |
| Editor de código embutido | Você já tem um. O AISENSE é o painel de controle, não a IDE |
| Colaboração multiusuário em tempo real | Muda tudo (auth, sync, conflito) por um ganho que o v1 não precisa |
| Automação que executa comando arbitrário | Superfície de ataque grande demais para o ganho. Ações de automação são um conjunto fechado |

---

## Sequência sugerida depois do v1

```
v1.0  Fases 00-09 (documentadas)  →  equipes, terminais, skills, barramento, quadro, orquestração
v1.1  Notas + Bancadas + Comandos do projeto + Gate de revisão      ← itens 🔴 que não couberem no v1
v1.2  Formações + Papéis + Conexões + Rotinas + Resumo de ausência
v1.3  Métricas + Git/PR + Orçamento
v2.0  Portais + Sprints + Integrações externas
```

**Sources:** [Maestri](https://www.themaestri.app/en) · [Maestri no Product Hunt](https://www.producthunt.com/products/maestri) · [Guia do Maestri (pt-BR)](https://github.com/arthurspk/guiadomaestri)
