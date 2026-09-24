# FASE 09 — Distribuição

**Objetivo:** qualquer pessoa consegue instalar e usar o AISENSE.

**Demonstração:** baixar o instalador, instalar, abrir, usar — nos três sistemas operacionais.

**Leitura obrigatória:** [11 — Segurança](../11-seguranca.md#atualizações) · [03 — Stack](../03-stack.md)

## Tarefas

### [ ] F09-01 — Empacotamento nos 3 SOs
`.dmg` (universal: Apple Silicon + Intel), `.msi` e `.exe` (NSIS), `.AppImage` e `.deb`.
Sidecars `aisense` e `aisense-mcp` incluídos e com permissão de execução.
**Aceite:** instalar em máquina limpa de cada SO e subir um agente `shell` com sucesso.

### [ ] F09-02 — Assinatura e notarização
Assinatura de código no macOS com notarização, assinatura Authenticode no Windows.
Segredos em GitHub Secrets, nunca no repositório.
**Aceite:** o app abre sem aviso de segurança no macOS e no Windows.

### [ ] F09-03 — Auto-update assinado
Tauri updater com verificação de assinatura, canal estável, UI de "atualização disponível",
opção de desligar.
**Aceite:** atualizar da versão N para N+1 preserva banco, configurações e skills do usuário.

### [x] F09-04 — Migração de dados entre versões
Runner de migração com backup automático do `.db` antes de aplicar, e rollback em caso de falha.
**Aceite:** migração que falha no meio restaura o backup e avisa o usuário sem perder dados.

> Feito: `Store::open` (`aisense-store/src/db.rs`) faz o backup (`VACUUM INTO`
> `aisense.db.bak-v<N>`) e migra numa **conexão única**; se a migração falhar, fecha a conexão,
> apaga `-wal`/`-shm` e copia o backup de volta (o backup fica, para uma segunda tentativa). Erros
> `MigrationRolledBack` e `RestoreFailed` (este diz onde está a cópia boa). O app
> (`main.rs`) esconde a janela, mostra um diálogo em pt-BR com o caminho do backup e sai.
> Teste: v1 com dados + migração 2 real + migração quebrada → volta à v1 com os dados e sem as
> tabelas da 2; depois a versão corrigida migra normalmente. A primeira versão usava o pool e
> falhava ~1 em 10: uma conexão ainda se fechando fazia checkpoint do WAL por cima do backup
> recolocado. O diálogo não foi visto numa janela real.

### [x] F09-05 — Diagnóstico exportável
"Exportar diagnóstico" gerando `.zip` com logs redigidos (tokens, chaves e caminho de home
mascarados), versões e adaptadores, com preview antes de salvar.
**Aceite:** teste garantindo que nenhum token ou chave aparece no pacote gerado.

> Feito: `aisense-core/src/diagnostics.rs` (`Redactor`, `DiagnosticBundle`, `.zip` *stored*
> escrito à mão — sem dependência nova; CRC-32 conferido contra o valor de referência). O app
> passou a gravar o próprio log em `logs/aisense-app.log` (a execução anterior vira `.1`);
> antes só ia para o terminal. Comandos `diagnostics_preview` (monta e guarda o pacote),
> `diagnostics_file_name` e `diagnostics_save` (grava **o pacote da prévia**, não um novo).
> Front: `features/settings/DiagnosticsDialog.tsx` — um arquivo por aba, o que fica de fora,
> "Salvar .zip…". Aceite: `no_token_or_key_reaches_the_generated_package` escreve o `.zip` e
> procura 12 segredos (keychain, `AISENSE_TOKEN`, chaves Anthropic/OpenAI/GitHub/Slack/AWS/Google,
> JWT, `Bearer`, `*_PASSWORD=`) e a home **nos bytes do arquivo** — como não há compressão, texto
> vazado apareceria literal — e lê o zip de volta para provar que o resto do log chegou. E2E do
> diálogo e axe nos dois temas; screenshots em `docs/screenshots/fase-09/`. Não visto no app
> real (janela).

### [ ] F09-06 — Documentação de usuário
README com instalação e primeiros passos, guia de criação de adaptador, guia de skills,
solução de problemas comuns.
**Aceite:** alguém que nunca viu o projeto instala e cria uma equipe funcional só com a documentação.

### [ ] F09-07 — Release automatizado
Workflow que, a partir de uma tag, compila nos 3 SOs, assina, publica o release no GitHub com
notas geradas a partir dos commits e atualiza o manifesto do updater.
**Aceite:** `git tag v0.1.0 && git push --tags` produz um release completo sem intervenção manual.

## Critérios de saída
- [ ] Instaladores assinados para macOS, Windows e Linux
- [ ] Auto-update funcionando de ponta a ponta
- [ ] Migração de dados segura com backup
- [ ] Documentação suficiente para um usuário autônomo
- [ ] Release por tag, sem passo manual

## Riscos
| Risco | Mitigação |
|---|---|
| Notarização da Apple falhar por entitlements | Resolver cedo, com um build de teste assinado ainda na Fase 8 |
| Sidecar perder permissão de execução no empacotamento | Teste de fumaça pós-instalação que executa `aisense whoami` |
| Antivírus do Windows sinalizar o binário | Assinatura Authenticode e submissão para análise se ocorrer |
