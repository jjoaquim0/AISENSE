# 06 — Sistema de Skills

> Uma **skill** é um pacote de instruções reutilizável que um agente carrega ao nascer.
> É o que faz o terminal subir já sabendo quem é, o que faz e como se comporta —
> sem você redigitar o mesmo prompt toda vez.

## Formato

Uma skill é um **diretório** com um `SKILL.md` (Markdown + frontmatter YAML) e, opcionalmente,
arquivos de apoio. É deliberadamente compatível com o formato de skills do Claude Code, para que
skills existentes funcionem sem conversão — ver [ADR 0005](adr/0005-skills-markdown.md).

```
revisor-rigoroso/
├── SKILL.md              # obrigatório
├── references/
│   └── checklist.md      # carregado sob demanda pela própria IA
└── scripts/
    └── diff-stats.sh     # opcional, executável
```

```markdown
---
name: revisor-rigoroso
description: Revisa diffs procurando bugs de correção, não estilo. Use antes de abrir PR.
version: 1.2.0
targets: [claude, codex, opencode]    # vazio ou ausente = compatível com todos
inject: bootstrap                     # bootstrap | reference | mcp
priority: 50                          # ordem de injeção; menor vem primeiro
env:
  REVIEW_STRICT: "1"
---

# Revisor rigoroso

Você revisa código procurando **defeitos de correção**, não preferências de estilo.

## Como trabalhar
1. Leia o diff inteiro antes de comentar qualquer coisa.
2. Para cada achado, descreva o cenário concreto de falha (entrada → saída errada).
3. Se não conseguir descrever a falha, não é um achado. Descarte.

## O que ignorar
Formatação, nomes subjetivos, preferências de organização — o linter cuida disso.

## Checklist completo
Consulte `references/checklist.md` quando o diff tocar em concorrência ou I/O.
```

### Campos do frontmatter

| Campo | Obrigatório | Significado |
|---|---|---|
| `name` | ✅ | Slug único, `^[a-z][a-z0-9-]*$` |
| `description` | ✅ | Uma frase dizendo **quando** usar. É o que a IA lê para decidir se aplica |
| `version` | — | SemVer, default `1.0.0` |
| `targets` | — | Lista de `adapter_id` compatíveis. Vazio = todos |
| `inject` | — | `bootstrap` (entra no prompt inicial) · `reference` (só materializa o arquivo) · `mcp` (exposta como ferramenta) |
| `priority` | — | Ordem de injeção (0–100, default 50) |
| `env` | — | Variáveis adicionadas ao PTY quando a skill está ativa |

## Ciclo de vida

```
 Biblioteca (~/.aisense/skills + embutidas)
        │  usuário atribui skill ao agente (tabela agent_skills, com ordem)
        ▼
 Boot do agente
   1. resolve   → lê SKILL.md, valida frontmatter, checa targets vs adapter
   2. materializa → copia para <workdir>/.aisense/agents/<handle>/skills/<name>/
                    e, se o adaptador tiver skills.dir, também para lá (ex.: .claude/skills/)
   3. compõe    → gera <workdir>/.aisense/agents/<handle>/BOOT.md
   4. injeta    → pelo melhor caminho que o adaptador suportar:
                  a) system_prompt_flag  (preferido: não polui o histórico)
                  b) MCP                 (skill exposta como recurso/ferramenta)
                  c) stdin               (fallback universal: "leia .aisense/agents/<handle>/BOOT.md e siga")
        ▼
 Agente rodando com as skills ativas
        │  usuário edita a skill na biblioteca
        ▼
 Aviso na UI: "3 agentes usam esta skill. Reiniciar para aplicar?" [Reiniciar] [Depois]
```

**Regra:** skills **não** são aplicadas a quente. Mudança de skill exige reinício do agente.
Tentar reinjetar no meio de uma conversa confunde a IA e torna o comportamento irreprodutível.

## Anatomia do `BOOT.md` gerado

Este é o arquivo que faz o agente "nascer sabendo". Gerado do template
`crates/aisense-core/src/skill/boot.rs` (o texto canônico de comunicação fica em
`skills/trabalho-em-equipe/SKILL.md`):

```markdown
# Você é @backend — Squad Produto

## Identidade
- Seu endereço no barramento: **@backend**
- Sua equipe: **Squad Produto**
- Missão da equipe: Migrar a autenticação para OAuth sem downtime.
- Seu papel: Implementa e mantém os serviços de API em Rust.
- Diretório de trabalho: /Users/voce/projeto

## Sua equipe
| Endereço | Papel | Runtime |
|---|---|---|
| @arquiteto | Define contratos e decide trade-offs | claude |
| @frontend | Implementa a interface em React | opencode |
| @revisor | Revisa antes de qualquer merge | codex |

## Como falar com a equipe
<conteúdo da skill embutida "trabalho-em-equipe" — ver abaixo>

## Suas skills
### revisor-rigoroso (v1.2.0)
<conteúdo do SKILL.md>

### rust-idiomatico (v2.0.1)
<conteúdo do SKILL.md>
```

Limite de tamanho: se o `BOOT.md` passar de **12.000 caracteres**, o compositor limita
campos de identidade longos e a tabela de colegas, inclui `name` + `description` + a
primeira seção de cada skill `bootstrap` e adiciona
"o conteúdo completo está em `.aisense/agents/<handle>/skills/<name>/SKILL.md`, leia sob demanda".
Se ainda não couber, omite skills do fim da lista com aviso e aponta para a pasta
materializada. Skills `reference` e `mcp` aparecem com descrição e caminho, sem o corpo.
Isso evita estourar a janela de contexto logo no boot. O limite usa caracteres Unicode.

## A skill embutida `trabalho-em-equipe`

É a skill que ensina qualquer IA a usar o barramento. Vai em **todo** agente automaticamente
(não é opcional, não aparece na lista de atribuição). Conteúdo canônico — implementar em
`skills/trabalho-em-equipe/SKILL.md`:

```markdown
---
name: trabalho-em-equipe
description: Como conversar com os outros agentes da sua equipe no AISENSE.
version: 1.0.0
priority: 0
---

# Trabalho em equipe

Você faz parte de uma equipe de agentes. Cada um roda no próprio terminal e tem um endereço
começando com `@`. Você fala com eles pelo comando `aisense`, que já está no seu PATH.

## Comandos

| Comando | Quando usar |
|---|---|
| `aisense agents` | Ver quem está na equipe, o papel e o estado de cada um |
| `aisense send @alguem "texto"` | Avisar algo. Não espera resposta |
| `aisense ask @alguem "pergunta"` | Perguntar e **aguardar a resposta** (bloqueia até 5 min) |
| `aisense reply <id> "texto"` | Responder uma pergunta que te fizeram |
| `aisense inbox` | Ler suas mensagens pendentes |
| `aisense inbox --drain` | Ler e marcar todas como lidas |
| `aisense wait --timeout 120` | Ficar parado até chegar mensagem |
| `aisense broadcast "texto"` | Avisar a equipe inteira |
| `aisense task add "título" --assign @alguem` | Criar tarefa no quadro |
| `aisense task list --mine` | Ver suas tarefas |
| `aisense task done <id>` | Concluir tarefa |
| `aisense note "texto"` | Registrar na linha do tempo, sem destinatário |

## Regras de convivência

1. **Cheque a caixa de entrada** no início de cada turno e ao terminar uma tarefa longa: `aisense inbox`.
2. **Responda o que te perguntarem.** Um `ask` sem resposta trava o colega até o timeout.
3. **Uma mensagem, um assunto.** Seja específico: diga o arquivo, a função, o erro exato.
4. **Não peça o que você mesmo consegue.** Ler um arquivo do projeto é com você;
   pergunte quando a informação estiver na cabeça do outro (uma decisão, um estado, um resultado).
5. **Não entre em laço.** Se você e outro agente trocarem 5 mensagens sobre a mesma coisa sem
   avançar, pare e use `aisense note` descrevendo o impasse para o humano resolver.
6. **Avise quando terminar** algo que outro agente esteja esperando.

## Exemplo

```bash
aisense agents
# @arquiteto  (claude, ocioso)   Define contratos
# @frontend   (opencode, ocupado) Implementa a UI

aisense ask @arquiteto "o endpoint /users v2 mantém o campo legacy_id?"
# → "Não. Removemos na v2. Use external_id."

aisense send @frontend "contrato do /users v2 subiu no branch feat/oauth, legacy_id foi removido"
aisense task done tsk_01J8XYZ
```
```

## Biblioteca de skills embutidas do v1

| Skill | Para quê |
|---|---|
| `trabalho-em-equipe` | Comunicação entre agentes (sempre ativa) |
| `coordenador` | Papel de coordenador: quebra objetivo em tarefas e distribui |
| `revisor-rigoroso` | Revisão focada em correção |
| `implementador` | Implementa tarefa pequena e avisa o revisor ao terminar |
| `pesquisador` | Pesquisa e publica achados estruturados em canal |
| `sintetizador` | Consome canal e produz relatório consolidado |
| `documentador` | Mantém documentação em dia com o código |

## Editor de skills (na UI)

- Markdown com preview lado a lado, fonte mono, realce de frontmatter.
- Validação ao vivo: frontmatter inválido, `name` duplicado, `targets` inexistente.
- Contador de caracteres com aviso ao aproximar do limite do `BOOT.md`.
- Botão **Testar em agente descartável**: sobe um agente temporário com só essa skill.
- Import/export: diretório ou `.zip`.

Regras do editor (F04-08, `aisense-core/src/skill/editor.rs`):

- Só escreve dentro de `~/.aisense/skills/`; caminho vindo da UI que aponte para fora é recusado.
  A gravação é atômica (temporário + rename), para o hot-reload nunca ler meio arquivo.
- **Embutidas são só leitura.** "Duplicar para editar" abre uma cópia `<nome>-copia` como skill nova.
- **O `name` não muda na edição**: as atribuições casam a skill por ele. Para outro nome, duplique.
- `targets` com runtime que o catálogo de adaptadores não conhece impede salvar; nome igual ao de
  uma embutida só avisa (a do usuário vence).
- Excluir apaga a pasta; as atribuições ficam, marcadas como "fora do disco" (F04-02).
- "Precisam reiniciar" = agentes com a skill **ligada** e processo vivo; os parados só pegam a
  mudança no próximo início. A barra do editor lista cada um e oferece reiniciá-los.

## Precedência quando há conflito

Ordem de composição no `BOOT.md` (do mais genérico ao mais específico — o último tem a última palavra):

```
1. trabalho-em-equipe (priority 0, sempre)
2. missão da equipe
3. papel do agente
4. skills atribuídas, por priority e depois por position
```
