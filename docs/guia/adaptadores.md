# Guia — Criar um adaptador para uma IA nova

> Um **adaptador** ensina o AISENSE a rodar uma CLI de IA num terminal: que comando chamar, como
> saber se ela está livre ou ocupada e como entregar mensagens a ela. É um arquivo TOML — não
> precisa recompilar nada. A referência completa do formato está em
> [05 — Adaptadores de Runtime](../05-adaptadores-runtime.md); aqui está o caminho prático.

## 1. Onde o arquivo mora

| Sistema | Pasta dos seus adaptadores |
|---|---|
| macOS e Linux | `~/.aisense/adapters/` |
| Windows | `%APPDATA%\AISENSE\adapters\` |

O caminho exato aparece em **Configurações → Runtimes → Pasta de adaptadores**. Um arquivo seu com o
mesmo `id` de um embutido (`claude`, `codex`, `opencode`, `gemini`, `shell`, `custom`) **substitui**
o embutido — é assim que você corrige uma flag que mudou numa versão nova da CLI.

O app observa a pasta: salvou o arquivo, a lista de runtimes se atualiza sozinha. Agentes que já
estão rodando continuam com o adaptador com que subiram até você reiniciá-los.

## 2. O mínimo que funciona

Crie `~/.aisense/adapters/minha-ia.toml`:

```toml
id          = "minha-ia"            # minúsculas, números e hífen
name        = "Minha IA"
description = "CLI da Minha IA"

command      = "minha-ia"           # o executável, procurado no PATH
args         = []
detect       = { command = "minha-ia", args = ["--version"] }
install_hint = "npm i -g minha-ia"  # aparece quando o comando não é encontrado

[state]
# Uma linha que só aparece quando a IA está esperando você digitar.
idle_regex = '(?m)^\s*>\s*$'
quiet_ms   = 400
```

Abra **Configurações → Runtimes**. A Minha IA aparece na lista com a versão que o `detect` devolveu,
ou como indisponível com o motivo e a dica de instalação.

Um erro no arquivo (campo com nome errado, regex que não compila, `id` inválido) aparece na mesma
tela com **arquivo e linha**. Campo desconhecido é erro de propósito: um `idle_regx` digitado errado
desligaria o detector sem aviso.

## 3. Estado: ensinar o app a ver quando a IA terminou

O AISENSE lê a tela do terminal (as últimas linhas, sem cores) e decide o estado do agente:

| Regex | Quando casa | O que o app faz |
|---|---|---|
| `awaiting_regex` | a IA pediu confirmação (`(y/n)`, "permission") | nunca injeta nada; avisa você |
| `busy_regex` | a IA está trabalhando ("thinking", "esc to interrupt") | espera |
| `idle_regex` | o prompt vazio esperando entrada | pode entregar mensagens (modo `push`) |

O jeito certo de acertar os regex é o **modo calibração**:

1. Crie uma equipe de teste com um agente usando o seu adaptador e inicie-o.
2. Em **Configurações → Runtimes → Modo calibração**, escolha o agente. A tela dele aparece como os
   regex a enxergam, com o estado que decidiriam e o erro de cada regex.
3. Ajuste até o veredito dizer **ocioso** quando o prompt está esperando você, e **ocupado** enquanto
   a IA responde.
4. **Aplicar sem reiniciar** grava no seu arquivo (comentários do TOML não são mantidos) e já vale
   para os agentes rodando.

Sem nenhum regex o app ainda funciona: depois de 60 s de silêncio marca o agente como ocioso com
confiança baixa e as mensagens ficam na caixa de entrada (modo `pull`) em vez de serem digitadas.

## 4. Mensagens e o BOOT.md

Para o agente nascer sabendo quem é, o AISENSE escreve um `BOOT.md` e o entrega pelo melhor caminho
que o adaptador declarar, nesta ordem:

```toml
[capabilities]
system_prompt_flag = "--append-system-prompt"  # 1º: o BOOT.md vai como argumento dessa flag
mcp                = true                      # 2º: com mcp_config, vai pelo servidor MCP
mcp_config         = ".mcp.json"               #     arquivo do projeto onde o app registra o aisense-mcp
model_flag         = "--model"                 # usada quando o agente tem um modelo escolhido

[inject]
mode      = "stdin"   # 3º: digita "leia .aisense/agents/<handle>/BOOT.md" no primeiro prompt
submit    = "\r"
prefix    = "[AISENSE] "
max_chars = 4000
```

Com `mcp_config`, o AISENSE registra o `aisense-mcp` nesse arquivo do projeto a cada início do
agente — não há passo manual. Sem nenhum dos três, o arquivo fica em
`.aisense/agents/<handle>/BOOT.md` para quem quiser ler.

Seja qual for o adaptador, todo terminal de agente recebe `aisense` e `aisense-mcp` no `PATH` e as
variáveis `AISENSE_*` — então a IA sempre pode rodar `aisense inbox`, `aisense send` etc. Se ela
não souber disso sozinha, o `BOOT.md` ensina.

## 5. Variáveis e segredos

```toml
[env]
MINHA_IA_TELEMETRY = "0"
```

Chaves de API **não** vão no TOML: guarde-as em **Configurações → Segredos**, que usa o keychain do
sistema e as injeta só no terminal dos agentes daquele runtime. Variáveis `AISENSE_*` são recusadas
(são a identidade do agente no barramento).

## 6. Conferir de ponta a ponta

1. O runtime aparece como disponível em **Configurações → Runtimes**.
2. Um agente com ele sobe, e o inspetor do agente (duplo clique no nome) mostra na linha **Boot** por
   onde o `BOOT.md` foi entregue.
3. De outro agente, `aisense send @seu-agente "oi"` aparece na vista **Mensagens** e, se o agente
   estiver em `push` e ocioso, é digitado no terminal dele.

Se algo não bate, veja [Solução de problemas](problemas.md).
