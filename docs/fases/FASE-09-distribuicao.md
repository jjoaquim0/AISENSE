# FASE 09 — Distribuição

**Objetivo:** qualquer pessoa consegue instalar e usar o AISENSE.

**Demonstração:** baixar o instalador, instalar, abrir, usar — nos três sistemas operacionais.

**Leitura obrigatória:** [11 — Segurança](../11-seguranca.md#atualizações) · [03 — Stack](../03-stack.md)

## Tarefas

### [~] F09-01 — Empacotamento nos 3 SOs
`.dmg` (universal: Apple Silicon + Intel), `.msi` e `.exe` (NSIS), `.AppImage` e `.deb`.
Sidecars `aisense` e `aisense-mcp` incluídos e com permissão de execução.
**Aceite:** instalar em máquina limpa de cada SO e subir um agente `shell` com sucesso.

> Parcial: Linux conferido aqui; macOS e Windows só no ensaio do workflow de release.
> `scripts/sidecars.mjs` compila os dois binários e os põe em `crates/aisense-app/binaries/` com o
> sufixo do target (universal via `lipo`); `tauri.bundle.conf.json` liga o `externalBin` — fora do
> `tauri.conf.json` porque o `tauri-build` exige os arquivos em disco, e o `cargo build` do dia a
> dia não deve depender deles. `pnpm app:build` faz os dois. Teste de fumaça
> `scripts/smoke-install.py`: sobe o app **instalado** com dados novos e um agente que roda
> `aisense whoami` e o `initialize` do `aisense-mcp` (Linux, macOS e Windows; `xvfb-run` sem
> tela). Aqui: `.deb` de 7,7 MB instalado com `apt` (`/usr/bin/aisense-app`, `aisense`,
> `aisense-mcp`, todos `0755`) e AppImage de 83 MB — a fumaça passou nos dois. Um agente `custom`
> no lugar do `shell` porque o teste precisa de um comando que termine sozinho; o caminho
> (supervisor → PTY → `PATH` com os sidecars → barramento) é o mesmo. **Falta:** `.dmg` e
> NSIS/MSI, e instalar em máquinas limpas dos três SOs.

### [~] F09-02 — Assinatura e notarização
Assinatura de código no macOS com notarização, assinatura Authenticode no Windows.
Segredos em GitHub Secrets, nunca no repositório.
**Aceite:** o app abre sem aviso de segurança no macOS e no Windows.

> Parcial: tudo o que é código está pronto; falta o que só o dono do projeto tem —
> os certificados. `release.yml` passa `APPLE_*` ao `tauri-action` (assina app e sidecars com
> *hardened runtime* e notariza) e, no Windows, importa o `.pfx` de `WINDOWS_CERTIFICATE` e passa
> a impressão digital ao empacotador (`tauri.signing.conf.json`, gerado no job), com carimbo de
> tempo. Sem os segredos o build sai sem assinatura **com aviso** no job. Passo a passo para gerar
> e cadastrar cada segredo em `docs/distribuicao.md`. **Falta:** conta Apple Developer e
> certificado Authenticode cadastrados, e o aceite (abrir sem aviso) conferido num Mac e num
> Windows limpos.

### [~] F09-03 — Auto-update assinado
Tauri updater com verificação de assinatura, canal estável, UI de "atualização disponível",
opção de desligar.
**Aceite:** atualizar da versão N para N+1 preserva banco, configurações e skills do usuário.

> Parcial: em código e testado na interface; falta o aceite N → N+1 com dois releases reais.
> `tauri-plugin-updater` (dependência nova: é o updater oficial do Tauri, com verificação minisign).
> A chave pública fica em `plugins.updater.pubkey`, vazia no repositório e gravada no build de
> release por `scripts/release-config.mjs` a partir do segredo (o CLI também a exige para assinar
> — descoberto aqui: sem ela o `tauri build` falha com "Missing comment in public key"); sem chave
> o comando responde `updates_disabled` e nada vai à rede. Canal estável = `releases/latest/download/latest.json`.
> `update_check` guarda a versão achada; `update_install` baixa com progresso (`update:progress`),
> confere a assinatura, chama `commands::shutdown` (o mesmo do fechar da janela, agora idempotente:
> guarda quem rodava, para agentes, barramento e PTYs) e reinicia. Preferência nova
> `advanced.checkUpdates` (padrão ligado; arquivo antigo sem o campo também liga). Front:
> `features/updates/` — faixa abaixo da barra de título (Novidades, Instalar e reiniciar, Depois),
> checagem 4 s depois de abrir, grupo "Atualizações" em Configurações → Avançado. 3 E2E + axe.
> Achado: o plugin exige `plugins.updater` na config (senão o app não sobe) — está no
> `tauri.conf.json` com `pubkey` vazio. Sobre o aceite: o banco, as preferências e as skills moram
> em `~/.aisense` (`%APPDATA%\AISENSE`), fora do diretório de instalação, e o instalador não os
> toca; o banco novo migra com backup (F09-04). **Falta** ver isso acontecer com v0.1.0 → v0.1.1
> publicados. `requireSignedVersion` (contra *downgrade*) está ligado: o
> `tauri signer sign` avulso não grava a versão, mas a assinatura que o bundler faz grava
> (`version:0.1.0` no `.sig` do `.deb`). Conferido aqui: `tauri build` com chave de teste gerou
> `.deb`, `.rpm` e `.AppImage` com os `.sig`.

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

### [~] F09-06 — Documentação de usuário
README com instalação e primeiros passos, guia de criação de adaptador, guia de skills,
solução de problemas comuns.
**Aceite:** alguém que nunca viu o projeto instala e cria uma equipe funcional só com a documentação.

> Parcial: escrita; o aceite pede uma pessoa que nunca viu o projeto. README (instalação por SO,
> CLIs de IA, primeiros passos seguindo o onboarding real, desenvolvimento) e `docs/guia/`
> (`adaptadores.md`, `skills.md`, `problemas.md`), escritos conferindo cada botão e caminho no
> código. Divergências achadas no caminho: `docs/05` descrevia botões "Novo adaptador" e
> "Instalar integração MCP" que não existem (corrigido para o que o app faz); `docs/11` pede
> prévia antes de importar skill, e o app não tem (o guia avisa; fica com a F04-08).

### [~] F09-07 — Release automatizado
Workflow que, a partir de uma tag, compila nos 3 SOs, assina, publica o release no GitHub com
notas geradas a partir dos commits e atualiza o manifesto do updater.
**Aceite:** `git tag v0.1.0 && git push --tags` produz um release completo sem intervenção manual.

> Parcial: workflow escrito; nunca rodou. `.github/workflows/release.yml`: tag `v*` → confere que a
> tag bate com `Cargo.toml` e `tauri.conf.json` e que os segredos do updater existem → rascunho com
> notas de `scripts/release-notes.mjs` (por tipo de commit, com o ID da tarefa; testado aqui) →
> `tauri-action` nos 3 SOs (macOS universal, Ubuntu 22.04, Windows) com `latest.json` → fumaça no
> pacote de cada SO → publica só com os três verdes (tag com hífen vira pre-release, fora do
> canal estável). `workflow_dispatch` faz um **ensaio** sem publicar, com os pacotes como
> artefatos. **Falta:** os segredos cadastrados e a primeira tag.

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
