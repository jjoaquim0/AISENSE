# FASE 04 — Sistema de Skills

**Objetivo:** o agente nasce já sabendo quem é, o que faz e como se comporta.

**Demonstração:** criar uma skill "revisor-rigoroso", atribuir ao `@revisor`, iniciar o agente e ver
que ele já se apresenta com esse papel — sem você digitar nada.

**Leitura obrigatória:** [06 — Sistema de Skills](../06-sistema-de-skills.md) ·
[05 — Adaptadores](../05-adaptadores-runtime.md)

## Tarefas

### [ ] F04-01 — Parser e validador de skills
Ler `SKILL.md`, extrair frontmatter (`gray_matter`), validar campos, reportar erro com caminho e
linha. Carregar skills embutidas e de `~/.aisense/skills/`.
**Aceite:** frontmatter inválido gera erro legível, não pânico; skills sem `description` são rejeitadas.

### [ ] F04-02 — Registro e persistência de skills
Repositório de `skills` e `agent_skills` com ordem de injeção, hot-reload do diretório com `notify`.
**Aceite:** editar um `SKILL.md` em disco atualiza a biblioteca na UI sem reiniciar o app.
Depende de F04-01, F02-02.

### [ ] F04-03 — Resolução por agente
Dado um agente, resolver as skills habilitadas, filtrar por `targets` vs `adapter_id`, ordenar por
`priority` e depois `position`, reportar incompatibilidades.
**Aceite:** skill com `targets: [claude]` atribuída a um agente `codex` gera aviso na UI e é ignorada
no boot. Depende de F04-02.

### [ ] F04-04 — Materialização
Copiar as skills resolvidas para `<workdir>/.aisense/skills/` e, quando o adaptador declarar
`skills.dir`, também para lá (ex.: `.claude/skills/`). Limpar resíduos de skills removidas.
Escrever `.aisense/agent.json`.
**Aceite:** iniciar o agente duas vezes com skills diferentes não deixa arquivo órfão.
Depende de F04-03.

### [ ] F04-05 — Compositor do `BOOT.md`
Template com identidade, equipe, missão, papel, tabela de colegas, a skill `trabalho-em-equipe` e as
skills atribuídas. Truncagem inteligente acima de 12.000 caracteres.
**Aceite:** snapshot com `insta` do `BOOT.md` de um agente de exemplo; truncagem testada.
Depende de F04-04.

### [ ] F04-06 — Injeção no boot
Escolher o melhor caminho conforme as capacidades do adaptador: `system_prompt_flag` → MCP →
stdin (fallback "leia .aisense/BOOT.md e siga"). Registrar qual caminho foi usado.
**Aceite:** os 3 caminhos testados; o de stdin espera o primeiro `idle` antes de digitar.
Depende de F04-05, F03-01.

### [ ] F04-07 — Skills embutidas
Escrever `trabalho-em-equipe`, `coordenador`, `revisor-rigoroso`, `implementador`, `pesquisador`,
`sintetizador` e `documentador` conforme [06](../06-sistema-de-skills.md#biblioteca-de-skills-embutidas-do-v1).
**Aceite:** `trabalho-em-equipe` entra em todos os agentes automaticamente e não aparece na lista
de atribuição. Depende de F04-01.

### [ ] F04-08 — Biblioteca e editor de skills (T7)
Grid da biblioteca, editor Markdown com preview e validação ao vivo, contador de caracteres,
import/export, aviso de "N agentes precisam reiniciar", botão "testar em agente descartável".
**Aceite:** editar uma skill em uso mostra o aviso com a lista exata de agentes afetados.
Depende de F04-02.

## Critérios de saída
- [ ] Agente sobe com identidade e skills aplicadas, sem intervenção
- [ ] O preview do `BOOT.md` mostra exatamente o que será injetado
- [ ] Skills do Claude Code funcionam sem conversão
- [ ] Editar skill avisa quem precisa reiniciar

## Riscos
| Risco | Mitigação |
|---|---|
| `BOOT.md` estourar o contexto do modelo | Limite de 12.000 caracteres com truncagem por seção e aviso visível no editor |
| Fallback por stdin digitar antes do runtime estar pronto | Esperar o primeiro `idle` com confiança alta, com timeout de 30 s e fallback para aviso na UI |
| Materializar em `.claude/` sobrescrever arquivos do usuário | Nunca sobrescrever: mesclar `settings.json` e usar subdiretório dedicado para as skills do AISENSE |
