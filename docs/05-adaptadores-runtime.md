# 05 — Adaptadores de Runtime

> Um **adaptador** ensina o AISENSE a conversar com uma CLI de IA: como iniciar, como saber se está
> ocupada, como injetar prompt e mensagem, e o que ela suporta.
> Adaptadores são **arquivos TOML**. Adicionar uma IA nova nunca exige recompilar o app.

## Onde ficam

| Local | Origem | Precedência |
|---|---|---|
| Embutidos no binário | Mantidos pelo projeto | menor |
| `~/.aisense/adapters/*.toml` | Do usuário | **maior** (sobrescreve embutido de mesmo `id`) |

Hot-reload: o core observa o diretório com `notify`. Salvou o TOML, a UI atualiza. Agentes já rodando
mantêm o adaptador com que subiram até reiniciarem.

## Formato

```toml
# ~/.aisense/adapters/claude.toml
id          = "claude"
name        = "Claude Code"
description = "CLI oficial da Anthropic"
icon        = "sparkles"

# Como iniciar
command = "claude"
args    = []
# Detecção de instalação: se falhar, a UI mostra o runtime como indisponível com dica de instalação.
detect       = { command = "claude", args = ["--version"] }
install_hint = "npm i -g @anthropic-ai/claude-code"

[capabilities]
mcp                = true    # aceita servidor MCP → usamos aisense-mcp (melhor caminho)
system_prompt_flag = "--append-system-prompt"   # injeta bootstrap sem sujar o histórico
hooks              = true    # suporta hooks → habilita o modo de entrega 'hook'
model_flag         = "--model"
cwd_is_project     = true    # a IA trata o cwd como projeto
resume_flag        = "--continue"
mcp_config         = ".mcp.json"    # onde registrar o aisense-mcp (formato mcpServers); pré-aprovado em settings_file

[state]
# Heurística de detecção de estado. Ver "Calibração" abaixo.
idle_regex     = '(?m)^\s*(?:│\s*)?[>❯]\s*$'
busy_regex     = '(?i)(thinking|working|esc to interrupt)'
awaiting_regex = '(?i)(do you want|\(y/n\)|\[y/N\]|permission)'
quiet_ms       = 400

[inject]
# Como uma mensagem de outro agente entra neste terminal quando delivery_mode = "push"
mode      = "stdin"             # stdin | none
submit    = "\r"
prefix    = "[AISENSE] "
max_chars = 4000                # acima disso, grava em arquivo e injeta só o caminho
boot      = true                # o BOOT.md pode entrar por aqui (F04-06); false no shell puro

[skills]
# Onde este runtime espera encontrar skills nativamente (além de .aisense/agents/<handle>/skills/)
dir         = ".claude/skills"
format      = "claude-skill"
settings_file = ".claude/settings.json"   # usado para instalar o hook de inbox

[env]
# Variáveis sempre presentes neste runtime
CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC = "1"
```

### Regras de validação

- Campo desconhecido é **erro**, não é ignorado: `idle_regx` aponta a linha em vez de desligar o
  detector em silêncio.
- `id` segue `^[a-z][a-z0-9-]*$` (até 64); `name` e `command` obrigatórios.
- Os três regex de `[state]` precisam compilar; `quiet_ms` entre 50 e 10 000.
- `[env]` recusa chaves `AISENSE_*`, pelo mesmo motivo do agente (invariante I7 em `docs/04`).
- `command = "$SHELL"` significa "o shell padrão do sistema" e é resolvido pelo supervisor.
- Dois arquivos do usuário com o mesmo `id`: vale o primeiro em ordem alfabética; o outro vira aviso.

### Detecção

- O `detect` roda com `stdin` fechado e timeout de 3 s; todos os adaptadores em paralelo.
- Sucesso → disponível, com a primeira linha da saída como versão. Código de saída ≠ 0, timeout
  ou executável ausente → indisponível, com o motivo e o `install_hint`.
- Sem `detect`, basta `command` existir no `PATH`.
- No Windows a busca no `PATH` tenta as extensões de `PATHEXT` (`claude` → `claude.cmd`).
- O resultado fica em cache até o usuário pedir para procurar de novo ou o adaptador mudar.

## Adaptadores embutidos no v1

| `id` | Runtime | MCP | Flag de system prompt | Notas |
|---|---|---|---|---|
| `claude` | Claude Code | ✅ | `--append-system-prompt` | Melhor integração: MCP + hooks + skills nativas |
| `codex` | OpenAI Codex CLI | ✅ | via config de perfil | Injeção por stdin como fallback |
| `opencode` | OpenCode | ✅ | `--prompt` inicial | Verificar flags na versão instalada |
| `gemini` | Gemini CLI | ✅ | arquivo de contexto | Skills entram como arquivo de contexto |
| `shell` | Shell puro (`$SHELL`) | ❌ | — | O humano chama a IA que quiser; `aisense` já está no PATH |
| `custom` | Comando arbitrário | ❌ | — | Usuário define `command`/`args` na UI do agente |

Arquivos em [`adapters/`](../adapters/), embutidos no binário. Cabeçalho de cada TOML registra a
versão da CLI em que as flags foram conferidas. Particularidades:

- `codex` não tem flag de system prompt (lê `AGENTS.md` do projeto) e retomar é o subcomando
  `codex resume --last`, por isso fica sem `resume_flag`.
- `gemini` ainda **não foi conferido** numa instalação real.
- `custom` usa `command = "$AGENT_COMMAND"`: o supervisor usa o comando do agente. Não tem regex
  de estado nem injeção (`inject.mode = "none"`), e a detecção responde `perAgent`.
- `shell` usa `command = "$SHELL"` (PowerShell no Windows).
- No Windows, `which` só tenta nomes com extensões do `PATHEXT`: o npm instala também um script
  `codex` sem extensão (para bash) que o Windows não executa.

> **Atenção de manutenção:** flags de CLIs de terceiros mudam entre versões. O adaptador deve ser
> tratado como *configuração que pode quebrar*, nunca como contrato estável. Por isso a UI mostra
> o resultado de `detect` e permite editar o TOML sem sair do app (Configurações → Runtimes).

## Entrega do `BOOT.md` (F04-06)

Na subida, o supervisor escolhe o melhor caminho que o adaptador tem e registra qual usou
(linha "Boot" do inspetor, evento `agent:boot`):

1. `capabilities.system_prompt_flag` → o `BOOT.md` inteiro vai como argumento depois da flag.
2. `capabilities.mcp` **e** `capabilities.mcp_config` (o AISENSE registra o `aisense-mcp` no
   arquivo do projeto) → entregue como `instructions` na conexão MCP. O `codex` tem MCP, mas a
   configuração dele é global do usuário: o AISENSE não mexe nela, e ele segue pelo terminal.
3. `[inject] mode = "stdin"` com `boot = true` → no primeiro ocioso com confiança alta o
   supervisor digita `<prefix>Leia .aisense/agents/<handle>/BOOT.md e siga as instruções...` +
   `submit`. Nunca com o agente aguardando o humano; sem prompt em 30 s, desiste e avisa.
4. Senão, nada: o arquivo fica em `.aisense/agents/<handle>/BOOT.md` para quem quiser ler.

## Ambiente injetado em TODO agente

Independente do adaptador, todo PTY recebe:

```bash
AISENSE_SOCKET=/Users/voce/.aisense/run/aisense.sock   # ou \\.\pipe\aisense
AISENSE_TOKEN=<token efêmero da sessão>
AISENSE_AGENT_ID=agt_01J...
AISENSE_AGENT_HANDLE=backend
AISENSE_TEAM_ID=tem_01J...
AISENSE_TEAM_NAME=Squad Produto
AISENSE_WORKDIR=/Users/voce/projeto
AISENSE_BOOT_FILE=/Users/voce/projeto/.aisense/agents/backend/BOOT.md
PATH=<dir dos sidecars>:$PATH     # coloca `aisense` e `aisense-mcp` à mão
```

Isso é o que faz a comunicação entre agentes funcionar **em qualquer terminal**, inclusive no
`shell` puro: a IA (ou você) simplesmente roda `aisense`.

## Calibração do detector de estado

A heurística de "o agente está ocioso" é a parte mais frágil do sistema. Regras de implementação:

1. O estado só muda para `idle` se **as duas** condições valerem: silêncio por `quiet_ms`
   **e** a tela casar com `idle_regex`. Saída chegando é `busy` na hora.
2. A tela é lida **de baixo para cima**: decide a linha mais baixa que casar com algum regex.
   Na mesma linha, `awaiting_regex` tem **prioridade máxima**, depois `busy_regex`, depois
   `idle_regex`. Com `awaiting_input` o AISENSE **nunca** injeta nada — a decisão é do humano.
   Por que a posição: a tela guarda o passado. Num programa de linha, o "(s/n)" já respondido
   e o "compilando…" já terminado continuam visíveis acima do prompt novo; o que está por
   último é o que o programa disse por último.
3. Se nenhum regex casar por mais de 60 s com silêncio total, o estado vira `idle` com
   confiança `low`, e a política de entrega cai para `pull` automaticamente naquela rodada.
4. A saída avaliada é a **última tela**, reconstruída por um emulador de terminal (`vt100`) e
   sem códigos ANSI — não o log. Só as **últimas 12 linhas não vazias** contam: diálogos e
   prompts ficam no rodapé, e uma palavra como "permission" numa resposta antiga lá no alto
   não pode marcar o agente como aguardando. A tela acompanha o tamanho do painel.
5. Cada troca de estado é registrada em `tracing` no nível `debug` — a tela Configurações → Runtimes
   tem um "modo calibração" que mostra estado em tempo real para o usuário ajustar os regex.
6. Enquanto o agente está em `starting`, saída não o marca como `busy`: é o próprio boot. Ele sai
   de `starting` quando a tela casa com algum regex (inclusive `awaiting`, para diálogos de
   confiança/login na subida) ou pelo silêncio longo da regra 3.

Implementação: `aisense-core/src/state/detector.rs` (puro, testado com transcrições de cada
runtime) e uma tarefa por sessão no supervisor, que alimenta o detector com **toda** a saída —
inclusive a de painéis fechados, que não geram evento para a UI.

**Nunca** trate detecção de estado como certeza. Toda injeção passa pela fila e é cancelável.

## Como adicionar um runtime novo (guia do usuário, vira ajuda na UI)

1. Configurações → Runtimes → **Novo adaptador** (abre o TOML em um editor com validação).
2. Preencha `command` e `detect`; clique em **Testar** — o app roda o comando e mostra a saída.
3. Suba um agente descartável e abra o **modo calibração**; ajuste `idle_regex` até o indicador
   ficar verde quando o prompt está esperando você.
4. Se a CLI suportar MCP, aponte a configuração dela para o binário `aisense-mcp`
   (o botão **Instalar integração MCP** faz isso automaticamente quando `capabilities.mcp = true`).
5. Salve. O adaptador aparece na lista de runtimes ao criar agentes.
