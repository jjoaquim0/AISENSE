# FASE 08 — Distribuição

**Objetivo:** qualquer pessoa consegue instalar e usar o AISENSE.

**Demonstração:** baixar o instalador, instalar, abrir, usar — nos três sistemas operacionais.

**Leitura obrigatória:** [11 — Segurança](../11-seguranca.md#atualizações) · [03 — Stack](../03-stack.md)

## Tarefas

### [ ] F08-01 — Empacotamento nos 3 SOs
`.dmg` (universal: Apple Silicon + Intel), `.msi` e `.exe` (NSIS), `.AppImage` e `.deb`.
Sidecars `aisense` e `aisense-mcp` incluídos e com permissão de execução.
**Aceite:** instalar em máquina limpa de cada SO e subir um agente `shell` com sucesso.

### [ ] F08-02 — Assinatura e notarização
Assinatura de código no macOS com notarização, assinatura Authenticode no Windows.
Segredos em GitHub Secrets, nunca no repositório.
**Aceite:** o app abre sem aviso de segurança no macOS e no Windows.

### [ ] F08-03 — Auto-update assinado
Tauri updater com verificação de assinatura, canal estável, UI de "atualização disponível",
opção de desligar.
**Aceite:** atualizar da versão N para N+1 preserva banco, configurações e skills do usuário.

### [ ] F08-04 — Migração de dados entre versões
Runner de migração com backup automático do `.db` antes de aplicar, e rollback em caso de falha.
**Aceite:** migração que falha no meio restaura o backup e avisa o usuário sem perder dados.

### [ ] F08-05 — Diagnóstico exportável
"Exportar diagnóstico" gerando `.zip` com logs redigidos (tokens, chaves e caminho de home
mascarados), versões e adaptadores, com preview antes de salvar.
**Aceite:** teste garantindo que nenhum token ou chave aparece no pacote gerado.

### [ ] F08-06 — Documentação de usuário
README com instalação e primeiros passos, guia de criação de adaptador, guia de skills,
solução de problemas comuns.
**Aceite:** alguém que nunca viu o projeto instala e cria uma equipe funcional só com a documentação.

### [ ] F08-07 — Release automatizado
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
| Notarização da Apple falhar por entitlements | Resolver cedo, com um build de teste assinado ainda na Fase 7 |
| Sidecar perder permissão de execução no empacotamento | Teste de fumaça pós-instalação que executa `aisense whoami` |
| Antivírus do Windows sinalizar o binário | Assinatura Authenticode e submissão para análise se ocorrer |
