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

[skills]
# Onde este runtime espera encontrar skills nativamente (além de .aisense/skills/)
dir         = ".claude/skills"
format      = "claude-skill"
settings_file = ".claude/settings.json"   # usado para instalar o hook de inbox

[env]
# Variáveis sempre presentes neste runtime
CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC = "1"
```

## Adaptadores embutidos no v1

| `id` | Runtime | MCP | Flag de system prompt | Notas |
|---|---|---|---|---|
| `claude` | Claude Code | ✅ | `--append-system-prompt` | Melhor integração: MCP + hooks + skills nativas |
| `codex` | OpenAI Codex CLI | ✅ | via config de perfil | Injeção por stdin como fallback |
| `opencode` | OpenCode | ✅ | `--prompt` inicial | Verificar flags na versão instalada |
| `gemini` | Gemini CLI | ✅ | arquivo de contexto | Skills entram como arquivo de contexto |
| `shell` | Shell puro (`$SHELL`) | ❌ | — | O humano chama a IA que quiser; `aisense` já está no PATH |
| `custom` | Comando arbitrário | ❌ | — | Usuário define `command`/`args` na UI do agente |

> **Atenção de manutenção:** flags de CLIs de terceiros mudam entre versões. O adaptador deve ser
> tratado como *configuração que pode quebrar*, nunca como contrato estável. Por isso a UI mostra
> o resultado de `detect` e permite editar o TOML sem sair do app (Configurações → Runtimes).

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
PATH=<dir dos sidecars>:$PATH     # coloca `aisense` e `aisense-mcp` à mão
```

Isso é o que faz a comunicação entre agentes funcionar **em qualquer terminal**, inclusive no
`shell` puro: a IA (ou você) simplesmente roda `aisense`.

## Calibração do detector de estado

A heurística de "o agente está ocioso" é a parte mais frágil do sistema. Regras de implementação:

1. O estado só muda para `idle` se **as duas** condições valerem: silêncio por `quiet_ms`
   **e** o fim do buffer casar com `idle_regex`.
2. `awaiting_regex` tem **prioridade máxima**: se casar, o estado é `awaiting_input` e o AISENSE
   **nunca** injeta nada — a decisão é do humano.
3. Se nenhum regex casar por mais de 60 s com silêncio total, o estado vira `idle` com flag
   `low_confidence`, e a política de entrega cai para `pull` automaticamente naquela rodada.
4. A saída avaliada é a **última tela** (linhas visíveis), com códigos ANSI removidos, não o log inteiro.
5. Cada troca de estado é registrada em `tracing` no nível `debug` — a tela Configurações → Runtimes
   tem um "modo calibração" que mostra estado em tempo real para o usuário ajustar os regex.

**Nunca** trate detecção de estado como certeza. Toda injeção passa pela fila e é cancelável.

## Como adicionar um runtime novo (guia do usuário, vira ajuda na UI)

1. Configurações → Runtimes → **Novo adaptador** (abre o TOML em um editor com validação).
2. Preencha `command` e `detect`; clique em **Testar** — o app roda o comando e mostra a saída.
3. Suba um agente descartável e abra o **modo calibração**; ajuste `idle_regex` até o indicador
   ficar verde quando o prompt está esperando você.
4. Se a CLI suportar MCP, aponte a configuração dela para o binário `aisense-mcp`
   (o botão **Instalar integração MCP** faz isso automaticamente quando `capabilities.mcp = true`).
5. Salve. O adaptador aparece na lista de runtimes ao criar agentes.
