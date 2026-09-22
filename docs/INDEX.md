# Documentação do AISENSE

> Se você é um agente de IA e vai desenvolver aqui: comece por [`../AGENTS.md`](../AGENTS.md),
> depois [`ESTADO.md`](ESTADO.md). Este índice é o mapa de tudo.

## Ordem de leitura recomendada

Para entender o projeto pela primeira vez, leia nesta sequência (≈45 min):

1. [01 — Visão de Produto](01-visao-produto.md) — o que é, para quem, o que não é
2. [02 — Arquitetura](02-arquitetura.md) — como as peças se encaixam
3. [03 — Stack Tecnológica](03-stack.md) — o que usamos e por quê
4. [07 — Barramento de Comunicação](07-barramento-comunicacao.md) — o coração do produto
5. [09 — Telas e Fluxos](09-telas-e-fluxos.md) — como o usuário vive isso

## Referência completa

### Produto e arquitetura
| Doc | Conteúdo |
|---|---|
| [01 — Visão de Produto](01-visao-produto.md) | Problema, público, princípios, casos de uso, escopo e não-escopo |
| [02 — Arquitetura](02-arquitetura.md) | Camadas, crates, fluxo de dados, ciclo de vida, performance |
| [03 — Stack Tecnológica](03-stack.md) | Escolhas de linguagem/biblioteca com alternativas comparadas |

### Núcleo técnico
| Doc | Conteúdo |
|---|---|
| [04 — Modelo de Dados](04-modelo-de-dados.md) | Entidades, esquema SQLite, migrações, layout em disco |
| [05 — Adaptadores de Runtime](05-adaptadores-runtime.md) | Como Claude/Codex/OpenCode/shell são integrados via TOML |
| [06 — Sistema de Skills](06-sistema-de-skills.md) | Formato, resolução, materialização e injeção no boot |
| [07 — Barramento de Comunicação](07-barramento-comunicacao.md) | Protocolo, IPC, CLI `aisense`, MCP, entrega de mensagens |
| [11 — Segurança](11-seguranca.md) | Segredos, permissões, sandbox, superfície de ataque |

### Interface
| Doc | Conteúdo |
|---|---|
| [08 — Design System](08-design-system.md) | Tokens OKLCH, tipografia, temas, componentes, movimento |
| [09 — Telas e Fluxos](09-telas-e-fluxos.md) | Cada tela desenhada, incluindo os 4 modos da Sala da Equipe |

### Processo
| Doc | Conteúdo |
|---|---|
| [10 — Padrões de Código](10-padroes-de-codigo.md) | Convenções Rust/TS, erros, testes, commits, PRs |
| [12 — Glossário](12-glossario.md) | Vocabulário canônico do projeto |
| [ESTADO.md](ESTADO.md) | **Estado vivo**: fase atual, progresso, decisões pendentes |

### Decisões (ADR)
| ADR | Decisão |
|---|---|
| [0000](adr/0000-template.md) | Template |
| [0001](adr/0001-tauri-em-vez-de-electron.md) | Tauri 2 em vez de Electron |
| [0002](adr/0002-core-em-rust.md) | Core de domínio e PTY em Rust |
| [0003](adr/0003-sqlite-local-first.md) | SQLite local-first, sem nuvem no v1 |
| [0004](adr/0004-protocolo-do-barramento.md) | Protocolo do barramento: NDJSON sobre socket local |
| [0005](adr/0005-skills-markdown.md) | Skills em Markdown + frontmatter, compatível com Claude Code |
| [0006](adr/0006-entrega-de-mensagens.md) | Entrega híbrida: caixa de entrada + injeção no PTY |

### Fases de desenvolvimento
| Fase | Título | Depende de |
|---|---|---|
| [00](fases/FASE-00-fundacao.md) | Fundação | — |
| [01](fases/FASE-01-terminal-core.md) | Terminal Core | 00 |
| [02](fases/FASE-02-equipes-agentes.md) | Equipes e Agentes | 01 |
| [03](fases/FASE-03-sala-da-equipe.md) | Sala da Equipe | 02 |
| [04](fases/FASE-04-skills.md) | Sistema de Skills | 02 |
| [05](fases/FASE-05-barramento.md) | Barramento de Comunicação | 03, 04 |
| [06](fases/FASE-06-orquestracao.md) | Orquestração | 05 |
| [07](fases/FASE-07-acabamento.md) | Acabamento | 06 |
| [08](fases/FASE-08-distribuicao.md) | Distribuição | 07 |

Visão geral do plano e critérios de "pronto": [fases/README.md](fases/README.md).
