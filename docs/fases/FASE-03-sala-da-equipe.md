# FASE 03 — Sala da Equipe

**Objetivo:** gerenciar muitos terminais ao mesmo tempo sem se perder.

**Demonstração:** iniciar uma equipe de 4 agentes, ver os 4 terminais vivos no grid, alternar para
o modo Foco, reorganizar painéis arrastando, navegar por `⌘1..4`, reiniciar um agente e ver o
estado de cada um em tempo real.

**Leitura obrigatória:** [09 — Telas](../09-telas-e-fluxos.md#t4--sala-da-equipe-) ·
[05 — Adaptadores](../05-adaptadores-runtime.md#calibração-do-detector-de-estado) ·
[08 — Design System](../08-design-system.md#indicadores-de-estado-do-agente)

## Tarefas

### [x] F03-01 — Detector de estado do agente
Máquina de estados de [02 — Arquitetura](../02-arquitetura.md#máquina-de-estados-do-agente) usando
os regex do adaptador sobre a última tela com ANSI removido, mais o silêncio de `quiet_ms`.
Emitir `agent:state` com nível de confiança.
**Aceite:** testes com transcrições gravadas de cada runtime produzindo a sequência de estados
esperada; `awaiting_input` sempre tem prioridade.
> Feito: `aisense-core/src/state/`. Tela reconstruída com `vt100` (dependência nova: CLIs de IA
> redesenham a tela com movimento de cursor, e texto cru não serve). Regras em `docs/05`. O
> supervisor roda uma tarefa de detector por sessão, alimentada por `OutputSink::raw` (toda a
> saída, visível ou não); `agent:state` ganhou `confidence`. Duas descobertas dos testes viraram
> regra: a tela é lida de baixo para cima (pergunta respondida e "trabalhando" antigo ficam
> visíveis acima do prompt novo) e `busy` vence `idle` na mesma leitura.
> **Ressalva:** as transcrições são **sintéticas**, modeladas no formato que cada regex espera —
> não gravações reais. Calibrar com sessões de verdade (claude, codex, opencode, gemini) fica para
> quem tiver os runtimes instalados; o risco já está em `ESTADO.md`.

### [x] F03-02 — Painel de agente
`AgentPane`: cabeçalho (handle, runtime, estado, badge, menu `⋮`), borda de 3px na cor do agente,
terminal, estado vazio quando parado, indicador de foco.
**Aceite:** o menu `⋮` cobre reiniciar, parar, limpar, duplicar e configurar. Depende de F03-01, F01-05.
> Feito: `features/team-room/components/AgentPane.tsx` e `paneMenu.ts` (itens com posição fixa;
> o que não se aplica fica desabilitado — teste cobre os 6 estados). "Duplicar" é
> `duplicate_agent` no core (handle `-2`, `-3`..., "(cópia)", próxima cor livre); "Limpar"
> esvazia a tela e o ring buffer (`pty_clear`), mantendo o log em disco. `StatusDot` ganhou as
> formas do doc 08 e o "?" de confiança baixa. Conferido por screenshot nos dois temas
> (`#/dev`, com o painel parado). **Fora:** "Abrir no terminal do SO", que o doc 09 lista no
> menu mas o aceite não pede — fica para a Fase 08.

### [x] F03-03 — Vista Grid
Layouts `1/2/3/4/6/9` + livre, com dnd-kit para arrastar e redimensionar. Persistir layout por
equipe em `teams.layout`.
**Aceite:** com 9 painéis o app mantém ≥55 fps ao arrastar. Depende de F03-02.
> Feito: `features/team-room/{gridLayout.ts, components/GridView.tsx, PresetPicker.tsx,
> hooks/useGridLayout.ts}` e o comando `team_set_layout` (objeto JSON, até 64 KB, sem mexer em
> `updated_at`). Presets arrastam com dnd-kit (teclado incluso); o modo livre arrasta e
> redimensiona numa grade de 24×24 mexendo só no DOM até soltar, e o último painel mexido fica
> por cima. Painéis que não cabem no preset aparecem em "Fora da grade" com um clique para mostrar.
> **Medição** (bancada `#/dev/grid`, 9 xterm reais, Chromium headless **sem GPU**): com os
> terminais parados, arrastar fica em 60 fps nos presets e no modo livre; o único recorte abaixo
> de 55 é a primeira meia-segunda, e um controle sem arraste mostra a mesma queda — é da
> medição. Com os 9 terminais despejando saída, o ambiente sem GPU fica em ~3–10 fps **parado**,
> e arrastar não piora. Ou seja: o arraste não custa quadros, mas os 55 fps com 9 terminais
> ativos só podem ser confirmados numa máquina com GPU (`pnpm dev` → `#/dev/grid`).
> A bancada pegou dois defeitos do modo livre, corrigidos: painel solto sobre outro ficava
> escondido atrás, e o painel perdia a largura ao soltar (estilo apagado em vez de restaurado).

### [x] F03-04 — Vista Foco
Um terminal grande + tira de miniaturas com as últimas 4 linhas renderizadas fora do xterm
(componente leve atualizado a 2 fps).
**Aceite:** a miniatura não cria instância de xterm; CPU permanece baixa. Depende de F03-02.
> Feito: `features/team-room/components/{FocusView,MiniPreview,ViewPicker}.tsx`. As miniaturas
> são texto puro vindo do core: `agent_previews` lê as últimas linhas da **mesma tela** que o
> detector de estado (F03-01) já mantém em `vt100` — nenhum custo novo de emulação. Uma chamada
> para todos os agentes a cada 500 ms, pausada com a janela em segundo plano e sem re-render
> quando nada mudou. A tela fica após o processo morrer, então a miniatura de quem caiu mostra o
> erro. Vista escolhida salva em `teams.layout.view`. Teste monta a vista no jsdom e confere que
> não há `.xterm`/`canvas` nas miniaturas e que a busca é uma só a 2 fps.
> Corrigido de carona um defeito da F03-03: equipe sem layout salvo abria com a lista de agentes
> ainda vazia, ganhava o preset "1" e ficava presa nele.

### [x] F03-05 — Gestão de visibilidade
Integrar com `pty_set_visible`: painel fora da tela ou em vista que não o mostra é marcado invisível;
ao ficar visível, reidrata com snapshot.
**Aceite:** com 9 agentes ativos em modo Foco, apenas 1 emite eventos `pty:data`.
Depende de F03-03, F03-04, F01-07.
> Feito: o supervisor sobe agentes com `PtyManager::spawn_hidden` — **invisíveis por padrão**;
> antes todo agente emitia eventos mesmo sem painel. O `<Terminal />` liga os eventos só quando
> está de fato na tela (`IntersectionObserver` + janela em primeiro plano) via o novo `pty_show`,
> que liga a emissão e devolve o histórico com o lote travado; ao sumir, desmontar ou a janela ir
> para segundo plano, desliga. A reidratação passa por um portão (`HydrationGate`) que segura a
> saída ao vivo até o histórico ser escrito, para ela não aparecer acima dele.
> Aceite como teste em `aisense-pty` (9 sessões produzindo saída, um `show`: só ele emite, os
> outros guardam tudo no ring) e no front (contrato show/esconder/desmontar/segundo plano).
> **Limite conhecido:** um chunk que já estava no ring mas ainda não tinha passado pelo lote no
> instante do `show` pode aparecer duas vezes — janela de milissegundos, a mesma que o painel
> já tinha ao montar. Eliminá-la pede numerar a saída; fica anotado se aparecer na prática.

### [x] F03-06 — Controles da equipe
▶ Iniciar equipe (sobe os `autostart` em ordem, com escalonamento de 300 ms entre spawns),
⏸ Parar tudo, ⟳ Reiniciar tudo, com confirmação e feedback de progresso.
**Aceite:** iniciar uma equipe de 6 agentes não congela a UI em momento algum. Depende de F02-06.
> Feito: `aisense-core/src/supervisor/team.rs` (`start_team`, `stop_team`, `restart_team`, evento
> `team:progress`) e `features/team-room/components/TeamControls.tsx`. ▶ sobe os `autostart` na
> ordem da equipe com 300 ms entre spawns; ⏸ para todos e espera saírem; ⟳ reinicia quem estava
> de pé (sem ninguém de pé, é o mesmo que ▶). ⏸ e ⟳ pedem confirmação só quando há agente
> rodando. Barra de progresso "Iniciando 3/6 · @backend" alimentada pelos eventos — a interface
> nunca espera um spawn. Arquivar e excluir a equipe agora esperam os processos saírem.
> Aceite como teste: 6 agentes num runtime de **uma thread só**, com uma tarefa-relógio ao lado;
> a equipe sobe em ordem, com o intervalo, e o relógio continua batendo durante todo o start.

### [x] F03-07 — Sidebar de agentes
Lista com estado ao vivo, badge de mensagens pendentes (preparado para a Fase 5), reordenação por
arrastar, clique foca o painel, duplo clique abre o inspetor.
**Aceite:** a lista reflete mudanças de estado em <200 ms. Depende de F03-01.
> Feito: `features/team-room/components/AgentSidebar.tsx`, agora **na sidebar do shell** (doc 09),
> não mais numa coluna dentro da sala. A sala continua dona dos agentes e da seleção e coloca a
> lista lá por portal (`features/shell/slots.tsx`, `ShellSlot`); o inspetor usa o mesmo caminho.
> Estado ao vivo por `useLiveStates`: o evento `agent:state` atualiza a lista direto, sem a volta
> ao core que recarregava todas as equipes a cada mudança — teste mede a atualização (bem abaixo
> de 200 ms, na mesma renderização). Reordenar pela alça (mouse ou teclado, dnd-kit) grava a
> `position` via `agents_reorder`, que exige a equipe inteira exatamente uma vez e recusa lista
> velha; essa é a ordem da equipe (a de ▶), separada da ordem dos painéis da Grid. Clique
> seleciona/foca o painel; duplo clique abre o inspetor, que por ora mostra o cabeçalho do agente
> (estado, runtime, pasta, papel) — as abas vêm na F03-09. Badge `PendingBadge` pronto sobre o
> ponto, na cor do remetente, sem fonte até a Fase 05. Leitores de tela ouvem só o que pede
> atenção (aguardando, erro, parou); trabalhando/ocioso alternam demais para anunciar.
> **Não conferido na janela** (sem tela aqui).

### [x] F03-08 — Navegação por teclado
`⌘1..9`, `⌘G` (ciclar vistas), `⌘T`, `⌘W`, `⌘\`, `⌘B`, `Esc Esc` para sair do foco do terminal.
Camada de atalhos global respeitando o foco do xterm.
**Aceite:** com o terminal focado, `⌘1` funciona e não vaza a tecla para o shell. Depende de F03-03.
> Feito: `lib/{shortcuts.ts, useShortcuts.ts}` — um listener só, na **captura** da janela (antes
> do textarea do xterm), com camadas empilhadas (a sala sobre o shell) — e
> `features/team-room/shortcuts.ts` com a regra de cada atalho. `⌘N` foca o painel N da ordem
> dos painéis (a das miniaturas), traz de volta à grade quem estava fora e põe o teclado no
> terminal (`features/terminal/focus.ts`); dígito lido do `code` físico (AZERTY). `⌘W` tira o
> painel da grade e encolhe o preset — o agente segue rodando; `⌘\` sobe para o próximo preset
> (no Foco, volta à grade); `⌘G` alterna Grade/Foco; `⌘T` abre "Novo agente"; `⌘B` já existia.
> `Esc Esc` em 400 ms leva o foco ao painel (`data-agent-pane`, com anel); a primeira `Esc` ainda
> vai ao processo. Terminal com `role="application"` e a instrução de saída (doc 08).
> **Decisão (ADR 0007):** com o terminal focado, fora do macOS só `⌘1..9` são interceptados —
> `Ctrl+W`, `Ctrl+B`, `Ctrl+\`... continuam do shell. Atalhos não disparam dentro de diálogos.
> Aceite como teste: um "xterm" com textarea ouvindo teclas; `Ctrl+1` roda o atalho, fica com
> `defaultPrevented` e o textarea não vê a tecla. **Não conferido na janela.**

### [x] F03-09 — Inspetor do agente (T6)
Abas Visão, Config e Logs (as abas Skills e Caixa entram nas fases 4 e 5).
**Aceite:** a aba Logs mostra a transcrição com busca e exportação. Depende de F03-02.
> Feito: `features/team-room/components/{AgentInspector.tsx, inspector/{OverviewTab,LogsTab}.tsx}`
> no inspetor do shell. **Logs:** o log é um arquivo por agente com as sessões em sequência;
> cada sessão agora grava onde começa (`sessions.log_offset`, migração 0003) e vai até onde a
> seguinte começa — é o que separa "sessões anteriores". O core (`aisense-core/src/transcript.rs`)
> transforma a saída crua em texto aos pedaços (escapes somem, `\r` sobrescreve a linha — barra de
> progresso fica no estado final —, `\b` recua, UTF-8 e escapes partidos entre leituras). A tela
> recebe o final da sessão (até 1 MB de log, começando numa linha inteira); **exportar** grava a
> sessão inteira no arquivo escolhido no diálogo de salvar (`dialog:allow-save`), em streaming.
> Busca no front: sem diferenciar maiúsculas, marca as ocorrências (até 1000), Enter/⇧Enter e
> setas andam entre elas. Comandos `agent_sessions`, `session_transcript` e `session_export`,
> com a leitura de disco fora da thread dos comandos. Sessão cujo trecho saiu do log (rotação
> aos 200 MB, ou gravada antes da 0003) aparece como indisponível, sem inventar conteúdo.
> **Visão:** estado, tempo ativo (relógio só enquanto roda), PID, nº de sessões, runtime, pasta,
> papel e os últimos 20 eventos de estado desde que a janela abriu. Mensagens e tarefas entram
> nas Fases 05 e 06. **Config:** os campos do T5 inline — o formulário saiu do diálogo para
> `features/agents/AgentFormFields.tsx` (não `AgentForm.tsx`: colidiria com `agentForm.ts` no macOS e no Windows), usado pelos dois. Transcrição não é um emulador: TUIs que
> redesenham a tela inteira ficam repetitivas no texto. **Não conferido na janela.**

## Critérios de saída
- [ ] 9 terminais simultâneos com performance dentro do orçamento — **pendente de máquina com GPU** (`#/dev/grid`) de [03 — Stack](../03-stack.md#orçamento-de-performance-metas-verificáveis-na-fase-8)
- [ ] Estado de cada agente visível e correto — visível sim; "correto" pede calibrar os regex com sessões reais de cada runtime
- [x] Layout persistido por equipe (`teams.layout`, F03-03/F03-04)
- [x] Navegação inteiramente por teclado (F03-08; conferido em teste, não na janela)
- [x] Trocar de vista não recria processos (os processos vivem no core; a vista só monta terminais)

## Riscos
| Risco | Mitigação |
|---|---|
| Detecção de estado imprecisa | Nesta fase o estado é só informativo (nada depende dele ainda); a precisão crítica só é exigida na Fase 5 |
| Grid arrastável consumir muito CPU | Usar transform em GPU; evitar reflow; virtualizar o que estiver fora da viewport |
