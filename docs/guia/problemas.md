# Guia — Solução de problemas

> Antes de tudo: **Configurações → Avançado → Exportar diagnóstico…** mostra e salva um `.zip` com a
> versão, o sistema, os runtimes detectados e o log do app, com chaves, tokens e a sua pasta pessoal
> mascarados. Anexe-o a qualquer relato de problema.

## Onde as coisas ficam

| O quê | macOS e Linux | Windows |
|---|---|---|
| Dados do app (banco, preferências, logs) | `~/.aisense/` | `%APPDATA%\AISENSE\` |
| Log do app (esta execução / a anterior) | `~/.aisense/logs/aisense-app.log` / `.log.1` | `%APPDATA%\AISENSE\logs\` |
| Transcrição do terminal de cada agente | `~/.aisense/logs/<id do agente>.log` | idem |
| Arquivos por agente na pasta da equipe | `<pasta>/.aisense/agents/<handle>/` | idem |

Para usar outra pasta de dados (instalação portátil, testes), defina `AISENSE_HOME` antes de abrir o
app. Para um log mais detalhado: **Configurações → Avançado → Nível de log → Depuração** (vale na
próxima abertura) ou a variável `AISENSE_LOG=debug`.

## Instalação e primeira abertura

**macOS: "o app está danificado" ou "não pode ser aberto".** A versão não foi assinada e notarizada
(builds de desenvolvimento, ou um release feito sem os certificados). Clique com o botão direito no
app → **Abrir**, ou `xattr -dr com.apple.quarantine /Applications/AISENSE.app`. Releases oficiais
assinados abrem direto.

**Windows: "O Windows protegeu o computador" (SmartScreen).** Mesmo motivo: instalador sem assinatura
Authenticode. **Mais informações → Executar assim mesmo**.

**Linux: janela em branco ou cinza.** Alguns drivers de vídeo não se dão bem com a composição
acelerada do WebKitGTK. Abra com `WEBKIT_DISABLE_COMPOSITING_MODE=1 aisense-app`; se resolver,
coloque a variável no atalho do app. Se não resolver, o log do app diz qual página tentou carregar
(linha `página url=...`).

**Linux: o AppImage não abre.** Dê permissão de execução (`chmod +x AISENSE_*.AppImage`). Em
distribuições sem FUSE 2, instale `libfuse2` ou rode com `--appimage-extract-and-run`.

## Runtimes

**Um runtime aparece como indisponível.** O app procura o comando no `PATH` que ele herdou ao abrir.
Se você instalou a CLI depois de abrir o app, ou ela está num `PATH` que só o seu shell interativo
conhece (nvm, asdf, `~/.local/bin`), feche e abra o AISENSE a partir de um terminal onde o comando
funciona, e clique em **Procurar de novo** em **Configurações → Runtimes**. A linha do runtime mostra o
motivo (não encontrado, saiu com erro, passou de 3 s) e a dica de instalação.

**Uma CLI mudou as flags e o agente não sobe.** Copie o adaptador para a sua pasta de adaptadores e
corrija a flag — o seu arquivo vence o embutido. Veja o [guia de adaptadores](adaptadores.md).

## Agentes

**O agente fica em "iniciando" ou nunca vira "ocioso".** O detector não reconheceu o prompt da IA
(versão nova da CLI, tema diferente). Use o **modo calibração** em **Configurações → Runtimes** para
ajustar o `idle_regex` vendo a tela real do agente. Enquanto isso, nada se perde: as mensagens ficam
na caixa de entrada dele.

**O agente caiu.** O painel mostra as últimas linhas da saída (em geral a mensagem de erro da própria
CLI) e o código de saída. A política de reinício do agente (na aba **Config** do inspetor) decide se
ele volta sozinho. A transcrição completa fica na aba **Logs** do inspetor.

**A IA pede uma chave de API.** Guarde-a em **Configurações → Segredos**: vai para o keychain do
sistema e é injetada só nos agentes daquele runtime. No Linux isso precisa de um serviço de segredos
rodando (GNOME Keyring ou KWallet); sem ele, gravar falha com a explicação — o app nunca grava a
chave num arquivo como alternativa.

## Mensagens entre agentes

**`aisense: command not found` ou "este terminal não foi aberto pelo AISENSE".** O `aisense` é
feito para os terminais que o AISENSE abre: é lá que ele é posto no `PATH` junto com a identidade do
agente. Num terminal comum não há agente em nome de quem falar (no pacote `.deb` o comando existe em
`/usr/bin`, mas responde com essa explicação).

**`aisense` sai com código 3.** O app foi fechado, ou o terminal é de um agente de uma execução
anterior do app. Reinicie o agente.

**A mensagem chegou na linha do tempo mas a IA não viu.** No modo `pull` (o padrão), a IA lê a caixa
de entrada quando roda `aisense inbox` — o `BOOT.md` ensina a fazer isso. Para o app digitar a
mensagem no terminal dela, mude a entrega do agente para `push` (aba **Config** do inspetor); o app só
digita quando o agente está ocioso com confiança alta, nunca no meio de uma resposta nem quando a IA
está esperando você.

**O barramento pausou a conversa.** Dois agentes trocando mensagens sem parar batem nos limites do
barramento (taxa, repetição, orçamento por conversa) e a conversa pausa com um aviso e o botão
**Continuar**. Os limites estão em **Configurações → Barramento**.

## Dados

**"AISENSE não pôde abrir os seus dados" ao abrir uma versão nova.** A atualização do banco falhou e
o app **restaurou o backup** automaticamente: nada foi perdido. A mensagem mostra onde está o backup
(`aisense.db.bak-v<N>`). Volte para a versão anterior e relate o problema com o diagnóstico. Se a
mensagem disser que nem o backup pôde ser restaurado, feche o app e copie o arquivo de backup por cima
de `aisense.db`.

**Recomeçar do zero.** Com o app fechado, renomeie a pasta de dados (`~/.aisense` →
`~/.aisense-antigo`). As pastas das equipes não são tocadas; os `.aisense/` dentro delas são só
arquivos gerados para os agentes.
