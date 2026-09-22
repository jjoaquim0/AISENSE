# ADR 0005 — Skills em Markdown + frontmatter, compatíveis com Claude Code

- **Status:** aceito
- **Data:** 2026-09-22
- **Fase:** 00

## Contexto

Skills precisam ser criadas e editadas por **usuários**, versionadas em git, compartilhadas entre
pessoas e consumidas por **LLMs de fornecedores diferentes**.

## Opções consideradas

| Opção | Prós | Contras |
|---|---|---|
| **Markdown + frontmatter YAML em diretório** | Formato que LLM lê melhor; diff limpo em git; editável em qualquer editor; compatível com o formato de skills do Claude Code | Sem validação forte além do frontmatter |
| JSON/YAML estruturado | Validável com schema | LLM lê pior; edição hostil; instrução vira string gigante escapada |
| Skill como código (JS/Python) | Poderosa, dinâmica | Superfície de segurança enorme; skill deixa de ser dado e vira programa |
| Banco de dados com editor próprio | Controle total | Não versionável em git; não compartilhável como arquivo |

## Decisão

Skill é um **diretório** com `SKILL.md` (frontmatter YAML + corpo Markdown) e arquivos de apoio
opcionais. O formato é deliberadamente compatível com skills do Claude Code.

## Consequências

**Positivas**
- Skills existentes do ecossistema Claude Code funcionam sem conversão.
- Time versiona skills junto com o repositório do projeto.
- O corpo vai direto para o prompt — sem transformação, sem perda.
- Materialização para `.claude/skills/` deixa a skill nativa para o runtime que a suporta.

**Negativas / custos aceitos**
- Nada impede uma skill mal escrita. Mitigação: validação de frontmatter, preview e o botão
  "Testar em agente descartável".
- Skills importadas de terceiros podem conter instruções hostis. Mitigação: preview obrigatório
  antes de importar (ver `docs/11-seguranca.md`).

**O que passa a ser proibido**
- Skill executar código automaticamente no boot. Scripts em `scripts/` só rodam se a **IA** decidir
  executá-los, com as permissões normais do runtime dela.

## Quando revisitar

Se surgir um padrão aberto de "skill portátil" adotado pelos principais runtimes.
