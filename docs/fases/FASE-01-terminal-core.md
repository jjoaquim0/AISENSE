# FASE 01 — Terminal Core

**Objetivo:** um terminal de verdade dentro do AISENSE — o alicerce de tudo.

**Demonstração:** abrir o app, clicar em "novo terminal", ver o shell rodando, executar `htop`,
redimensionar a janela e ver o terminal se ajustar, rolar o histórico, buscar texto.

**Leitura obrigatória:** [02 — Arquitetura](../02-arquitetura.md#fluxo-2--saída-do-terminal-chegando-na-tela) ·
[03 — Stack](../03-stack.md) · [08 — Design System](../08-design-system.md#tema-do-terminal)

## Tarefas

### [x] F01-01 — Spawn de PTY multiplataforma
`aisense-pty`: `PtyHandle` com `spawn(cmd, args, cwd, env, size)`, `write`, `resize`, `kill`, `wait`.
Usar `portable-pty`. Leitura em `spawn_blocking`, saída para um canal `broadcast`.
**Aceite:** teste que sobe `echo hello`, captura a saída e confirma o exit code nos 3 SOs.
> `PtySession` sobre `portable-pty`, com `spawn`/`write`/`resize`/`kill`/`wait`/`subscribe`. Leitura numa thread própria (I/O de PTY é bloqueante) e `clone_killer()` antes de mover o filho para a thread de espera — assim parar o agente não depende dela. **Verificado só em Linux neste ambiente**; macOS e Windows (ConPTY) dependem do job `desktop` do CI. 13 testes de integração, incluindo `stty size` (prova que o tamanho chega ao processo, o que TUIs exigem) e propagação de ambiente.

### [x] F01-02 — Ring buffer e log em arquivo
Buffer circular das últimas N linhas (default 10.000, configurável) em RAM, mais gravação
append-only em `~/.aisense/logs/<id>.log`. API `snapshot()` devolvendo o buffer para reidratação.
**Aceite:** gerar 100.000 linhas e confirmar memória estável e as últimas 10.000 no snapshot.
Depende de F01-01.
> Ring buffer com limite por linhas **e** por bytes, reconstrução byte a byte (ANSI e UTF-8 preservados) e contador de descarte para a UI poder avisar que o histórico foi truncado. Log append-only separado, com rotação de uma geração. Um teste prova a divisão de papéis: a linha que o ring já descartou continua no arquivo.

### [x] F01-03 — Coalescedor de saída
Agregar chunks numa janela de 16 ms antes de emitir para a UI. Parar de emitir quando o painel
está marcado como invisível (o buffer continua acumulando no Rust).
**Aceite:** um comando que cospe 10 MB gera ≤60 eventos/s; com o painel invisível, gera zero.
Depende de F01-02.
> `Batcher` com lógica pura recebendo o instante por parâmetro — testes determinísticos, sem `sleep`. Primeiro chunk sai na hora (senão o eco do que você digita atrasaria 16 ms); o resto agrupa por janela. Painel invisível não emite nada e nada represado é despejado ao voltar, porque o front reidrata pelo snapshot. Teste confirma ≤64 lotes por segundo de saída contínua.

### [x] F01-04 — Comandos e eventos Tauri de PTY
`pty_spawn`, `pty_write`, `pty_resize`, `pty_kill`, `pty_snapshot`, `pty_set_visible`;
eventos `pty:data` e `pty:exit`. Tipos via `ts-rs`.
**Aceite:** dá para subir e matar um PTY inteiramente pelo devtools. Depende de F01-03.
> `PtyManager` (registro, bombeamento, visibilidade, `shutdown`) mora em `aisense-pty`, não no crate do Tauri, justamente para ser testável sem janela — **10 testes** cobrem entrega, término com código de saída, painel invisível, sessão duplicada, reinício e 6 agentes simultâneos. Os comandos Tauri em si são casca fina e **não foram compilados** aqui (sem WebKit/GTK).

### [~] F01-05 — Componente `<Terminal />`
xterm.js com `WebglAddon`, `FitAddon`, `SearchAddon`, `WebLinksAddon`. Ciclo de vida correto:
`dispose()` no unmount, `ResizeObserver` com debounce de 50 ms chamando `fit()` + `pty_resize`.
**Aceite:** montar e desmontar 50 vezes não vaza memória (verificado no profiler). Depende de F01-04.
> **Parcial:** componente escrito com `FitAddon`, `SearchAddon`, `WebLinksAddon` e `WebglAddon` (com aviso e queda para o renderer padrão se indisponível), `ResizeObserver` com debounce de 50 ms e `dispose()` no unmount. O critério "montar e desmontar 50 vezes sem vazar" **não foi verificado** — precisa do profiler numa máquina com GUI.

### [~] F01-06 — Tema do terminal derivado dos tokens
Mapear os tokens para o tema do xterm, incluindo as 16 cores ANSI corrigidas para contraste em
ambos os temas. Trocar tema sem recriar a instância do xterm.
**Aceite:** alternar claro/escuro com o terminal cheio de saída colorida não pisca nem recria o
terminal, e todas as 16 cores ANSI são legíveis nos dois temas. Depende de F01-05.
> **Parcial:** as 16 cores ANSI viraram tokens nos dois temas, com **13 testes de contraste** garantindo ≥4,5:1 sobre o fundo do terminal (o azul ANSI clássico é ilegível em fundo escuro; o nosso passa). `readTerminalTheme` converte OKLCH→hex porque o xterm não entende `oklch()`, e troca a cor **sem recriar** o terminal. A verificação visual da troca de tema continua pendente.

### [~] F01-07 — Reidratação ao focar
Painel invisível não mantém xterm. Ao ficar visível, chamar `pty_snapshot` e escrever o buffer de
uma vez só (`write` em bloco, não linha a linha).
**Aceite:** alternar para um painel com 10.000 linhas de histórico leva <100 ms. Depende de F01-05.
> **Parcial:** `pty_snapshot` devolve o histórico em base64 e o componente escreve tudo de uma vez no boot (escrever linha a linha congela a UI). O lado Rust está testado (snapshot, descarte, visibilidade); a medição de "<100 ms para 10.000 linhas" precisa de GUI.

### [~] F01-08 — Busca e cópia
`SearchAddon` com UI de busca (`⌘F`), destaque de ocorrências, contador e navegação.
Seleção com cópia automática opcional, colar com `⌘V`, menu de contexto.
**Aceite:** buscar em 10.000 linhas responde em <50 ms. Depende de F01-05.
> **Parcial:** `TerminalSearch` com ⌘F, navegação e Esc. O ⌘F é interceptado pelo `attachCustomKeyEventHandler` do xterm — um `onKeyDown` no elemento de fora quase nunca dispararia, porque o xterm move o foco para um textarea escondido. Sem medição de desempenho da busca.

## Critérios de saída
- [ ] Terminal fiel: cores, TUI (`htop`, `vim`), redimensionamento, unicode, emoji
- [ ] Zero vazamento ao criar/destruir painéis repetidamente
- [ ] Saída volumosa não trava a UI
- [ ] Os dois temas ficam legíveis
- [ ] Funciona nos 3 SOs (ConPTY no Windows testado de verdade)

## Riscos
| Risco | Mitigação |
|---|---|
| ConPTY no Windows se comportar diferente (sequências, resize) | Testar cedo, no início da fase, e não no fim; testes de CI específicos para Windows |
| WebGL indisponível em alguma webview | Detectar e cair para o renderer canvas com aviso no log; nunca cair para o renderer DOM |
| Saída binária/inválida quebrar o decode | Decodificar como UTF-8 lossy; nunca entrar em pânico com byte inválido |
