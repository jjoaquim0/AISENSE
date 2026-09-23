# FASE 04 — Sistema de Skills

**Objetivo:** o agente nasce já sabendo quem é, o que faz e como se comporta.

**Demonstração:** criar uma skill "revisor-rigoroso", atribuir ao `@revisor`, iniciar o agente e ver
que ele já se apresenta com esse papel — sem você digitar nada.

**Leitura obrigatória:** [06 — Sistema de Skills](../06-sistema-de-skills.md) ·
[05 — Adaptadores](../05-adaptadores-runtime.md)

## Tarefas

### [x] F04-01 — Parser e validador de skills
Ler `SKILL.md`, extrair frontmatter (`gray_matter`), validar campos, reportar erro com caminho e
linha. Carregar skills embutidas e de `~/.aisense/skills/`.
**Aceite:** frontmatter inválido gera erro legível, não pânico; skills sem `description` são rejeitadas.
> Feito: `aisense-core/src/skill/` (`parse.rs`, `catalog.rs`, `model.rs`). **Troca de
> dependência:** `yaml-rust2` em vez de `gray_matter` — o frontmatter é separado à mão (aceita
> BOM e `\r\n`) e o YAML lido com `yaml-rust2`, que dá a posição do erro de sintaxe; erro de
> campo aponta a linha da chave. Tudo vira `SkillProblem` com `caminho:linha`, nunca pânico
> (teste com lixo, números fora de faixa, arquivo de 256 KB+). Campos desconhecidos são
> **ignorados**, não recusados como nos adaptadores: skills do Claude Code trazem `license`,
> `allowed-tools`, `metadata` e têm de carregar sem conversão (critério de saída da fase).
> `version: 1.2` (número em YAML) é recusado com a dica de pôr aspas. `env` segue a regra dos
> agentes (sem `AISENSE_*`). Corpo vazio é recusado. `SkillCatalog` carrega as embutidas
> (`BUILTIN_SKILLS`, vazio até a F04-07) e `~/.aisense/skills/<pasta>/SKILL.md`
> (`DataDir::skills`); a do usuário vence a embutida de mesmo nome, nome repetido na pasta do
> usuário fica com a primeira em ordem alfabética e o resto vira aviso.

### [x] F04-02 — Registro e persistência de skills
Repositório de `skills` e `agent_skills` com ordem de injeção, hot-reload do diretório com `notify`.
**Aceite:** editar um `SKILL.md` em disco atualiza a biblioteca na UI sem reiniciar o app.
Depende de F04-01, F02-02.
> Feito: porta `SkillRepository` no core (`repo/mod.rs`), em memória e em SQLite
> (`aisense-store/src/skills.rs`), com o mesmo contrato testado contra os dois. O conteúdo da
> skill mora no disco; o banco guarda a **identidade estável** (`skills.id`, casada por `slug`)
> que as atribuições referenciam. `sync_skills` insere e atualiza, mas **não apaga** o que sumiu
> do disco: um `SKILL.md` com erro de digitação não pode levar as atribuições junto — a skill
> fica listada como "não está mais no disco". `agent_skills` troca a lista inteira numa
> transação, na ordem de injeção, e recusa agente ou skill inexistente sem mudar nada.
> Hot-reload: o observador dos adaptadores virou `fswatch::DirWatcher` (genérico, com o mesmo
> debounce) e o `SkillWatcher` observa `~/.aisense/skills/` recursivamente. No app,
> `SkillLibrary` guarda o catálogo, espelha no banco ao subir e a cada recarga, e emite
> `skills:changed`; comandos `skills_library`, `agent_skills_get`, `agent_skills_set`.
> UI: aba **Skills** do inspetor (T6) — as do agente em ordem, com liga/desliga, subir/descer
> e remover; o que dá para adicionar da biblioteca; os `SKILL.md` que não carregaram com
> `caminho:linha`; aviso de reinício com o agente rodando e de runtime incompatível (o filtro
> no boot é da F04-03). Aceite como teste: no core, criar/editar/apagar um `SKILL.md` recarrega
> o catálogo; no front, `skills:changed` refaz a lista sem reiniciar.

### [x] F04-03 — Resolução por agente
Dado um agente, resolver as skills habilitadas, filtrar por `targets` vs `adapter_id`, ordenar por
`priority` e depois `position`, reportar incompatibilidades.
**Aceite:** skill com `targets: [claude]` atribuída a um agente `codex` gera aviso na UI e é ignorada
no boot. Depende de F04-02.
> Feito: `aisense-core/src/skill/resolve.rs` (`resolve_agent_skills`). Entram as habilitadas que
> estão no disco e rodam no runtime; ordem por `priority` e, empatando, pela posição que o
> usuário deu (ordenação estável). As que ficam de fora viram `IgnoredSkill` com o motivo
> (`incompatible` com runtime e `targets`, ou `missing`) e uma frase pronta em pt-BR. O
> supervisor resolve em todo start (a biblioteca entrou no `SupervisorConfig`) e devolve o
> plano em `StartOutcome.skills`; ignorar **não impede** o start. Na UI: o start de um agente e
> o ▶ da equipe mostram as ignoradas junto das ressalvas de bancada, e a aba Skills do inspetor
> mostra "No próximo início" (ativas em ordem e ignoradas com o porquê) pelo comando novo
> `agent_skills_plan`, atualizado ao mexer na lista e a cada recarga da biblioteca. Aceite como
> teste no core (skill só do claude num agente codex: fora, com aviso; o supervisor sobe o
> agente e devolve a ignorada) e no front (a aba mostra a frase). As skills resolvidas ainda não
> chegam ao processo — isso é a materialização (F04-04) e a injeção (F04-06).

### [x] F04-04 — Materialização
Copiar as skills resolvidas para `<workdir>/.aisense/skills/` e, quando o adaptador declarar
`skills.dir`, também para lá (ex.: `.claude/skills/`). Limpar resíduos de skills removidas.
Escrever `.aisense/agent.json`.
**Aceite:** iniciar o agente duas vezes com skills diferentes não deixa arquivo órfão.
Depende de F04-03.
> Feito: `aisense-core/src/skill/materialize.rs`, chamado pelo supervisor em todo start, fora
> das threads assíncronas. **Mudança de layout (docs 02, 05 e 06 atualizados):** uma pasta por
> agente, `.aisense/agents/<handle>/{agent.json, skills/}` — no modo compartilhado (o padrão)
> vários agentes usam o mesmo diretório e um `agent.json` na raiz seria sobrescrito a cada start.
> As skills do agente são reescritas do zero a cada boot (a do usuário com os arquivos de apoio,
> sem links simbólicos nem `.git`; o `SKILL.md` é o texto já validado). `agent.json` traz
> identidade, equipe, missão e as skills em ordem — **sem o token** (R3). Handle renomeado: a
> pasta antiga do mesmo agente sai. Pasta nativa do runtime (`skills.dir`, ex. `.claude/skills/`):
> copia para lá **sem nunca sobrescrever** o que o AISENSE não criou (vira ressalva); o manifesto
> `.aisense/native-skills.json` guarda os donos de cada pasta, e ela só sai quando o último agente
> deixa de usá-la; `skills.dir` que aponte para fora do diretório é recusado. `.gitignore` em
> `.aisense/` escrito uma vez, ignorando tudo menos `notes/`. Falha de disco não impede o start:
> volta como ressalva em `StartOutcome.notes`, mostrada na UI e no relatório do ▶. Aceite como
> teste: dois inícios com skills diferentes deixam exatamente os arquivos da segunda lista, em
> `.aisense/` e em `.claude/skills/`. **Limite conhecido:** num diretório compartilhado, a pasta
> nativa é uma só — um agente claude enxerga lá as skills dos outros agentes claude da equipe
> (o `BOOT.md` de cada um, F04-05, só cita as dele). Com bancada própria (docs/16), fica isolado.

### [x] F04-05 — Compositor do `BOOT.md`
Template com identidade, equipe, missão, papel, tabela de colegas, a skill `trabalho-em-equipe` e as
skills atribuídas. Truncagem inteligente acima de 12.000 caracteres.
**Aceite:** snapshot com `insta` do `BOOT.md` de um agente de exemplo; truncagem testada.
Depende de F04-04.
> Feito: `skill/boot.rs` compõe o documento de cada agente com identidade, missão,
> papel, colegas em ordem da equipe e a skill `trabalho-em-equipe` (texto canônico em
> `skills/trabalho-em-equipe/SKILL.md`). Skills `bootstrap` entram com o corpo; as de
> referência/MCP aparecem com descrição e caminho. Ao ultrapassar 12.000 caracteres,
> limita campos longos, resume cada skill pela descrição e primeira seção e aponta
> para o arquivo completo; se necessário, omite entradas finais com aviso. O supervisor
> passa os colegas à materialização em todo start, que escreve o `BOOT.md` e devolve
> uma ressalva se o conteúdo foi resumido. Testes: snapshot com `insta`, limite e
> integração no start. A F04-06 fará a injeção; a F04-07 registrará a skill embutida
> na biblioteca para materialização automática.

### [ ] F04-06 — Injeção no boot
Escolher o melhor caminho conforme as capacidades do adaptador: `system_prompt_flag` → MCP →
stdin (fallback "leia .aisense/agents/<handle>/BOOT.md e siga"). Registrar qual caminho foi usado.
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

### [ ] F04-09 — Notas da equipe
Armazenamento em `<workdir>/.aisense/notes/`, leitura, `append` atômico (`O_APPEND`) e `write` com
trava otimista por hash. Índice das notas (título e resumo, não o conteúdo) no `BOOT.md`, dentro do
orçamento de 12.000 caracteres. Editor na UI reaproveitando o editor de skills.
Ver [15 — Notas da Equipe](../15-notas-da-equipe.md).
**Aceite:** dois agentes dando `append` na mesma nota ao mesmo tempo não perdem conteúdo (teste de
concorrência); `write` com hash desatualizado falha com `stale_note` e mostra o diff.
Depende de F04-05.

## Critérios de saída
- [ ] Agente sobe com identidade e skills aplicadas, sem intervenção
- [ ] O preview do `BOOT.md` mostra exatamente o que será injetado
- [ ] Skills do Claude Code funcionam sem conversão
- [ ] Editar skill avisa quem precisa reiniciar
- [ ] Notas da equipe legíveis e escrevíveis sem perda sob concorrência

## Riscos
| Risco | Mitigação |
|---|---|
| `BOOT.md` estourar o contexto do modelo | Limite de 12.000 caracteres com truncagem por seção e aviso visível no editor |
| Fallback por stdin digitar antes do runtime estar pronto | Esperar o primeiro `idle` com confiança alta, com timeout de 30 s e fallback para aviso na UI |
| Materializar em `.claude/` sobrescrever arquivos do usuário | Nunca sobrescrever: mesclar `settings.json` e usar subdiretório dedicado para as skills do AISENSE |
