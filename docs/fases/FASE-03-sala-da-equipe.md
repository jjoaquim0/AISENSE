# FASE 03 — Sala da Equipe

**Objetivo:** gerenciar muitos terminais ao mesmo tempo sem se perder.

**Demonstração:** iniciar uma equipe de 4 agentes, ver os 4 terminais vivos no grid, alternar para
o modo Foco, reorganizar painéis arrastando, navegar por `⌘1..4`, reiniciar um agente e ver o
estado de cada um em tempo real.

**Leitura obrigatória:** [09 — Telas](../09-telas-e-fluxos.md#t4--sala-da-equipe-) ·
[05 — Adaptadores](../05-adaptadores-runtime.md#calibração-do-detector-de-estado) ·
[08 — Design System](../08-design-system.md#indicadores-de-estado-do-agente)

## Tarefas

### [ ] F03-01 — Detector de estado do agente
Máquina de estados de [02 — Arquitetura](../02-arquitetura.md#máquina-de-estados-do-agente) usando
os regex do adaptador sobre a última tela com ANSI removido, mais o silêncio de `quiet_ms`.
Emitir `agent:state` com nível de confiança.
**Aceite:** testes com transcrições gravadas de cada runtime produzindo a sequência de estados
esperada; `awaiting_input` sempre tem prioridade.

### [ ] F03-02 — Painel de agente
`AgentPane`: cabeçalho (handle, runtime, estado, badge, menu `⋮`), borda de 3px na cor do agente,
terminal, estado vazio quando parado, indicador de foco.
**Aceite:** o menu `⋮` cobre reiniciar, parar, limpar, duplicar e configurar. Depende de F03-01, F01-05.

### [ ] F03-03 — Vista Grid
Layouts `1/2/3/4/6/9` + livre, com dnd-kit para arrastar e redimensionar. Persistir layout por
equipe em `teams.layout`.
**Aceite:** com 9 painéis o app mantém ≥55 fps ao arrastar. Depende de F03-02.

### [ ] F03-04 — Vista Foco
Um terminal grande + tira de miniaturas com as últimas 4 linhas renderizadas fora do xterm
(componente leve atualizado a 2 fps).
**Aceite:** a miniatura não cria instância de xterm; CPU permanece baixa. Depende de F03-02.

### [ ] F03-05 — Gestão de visibilidade
Integrar com `pty_set_visible`: painel fora da tela ou em vista que não o mostra é marcado invisível;
ao ficar visível, reidrata com snapshot.
**Aceite:** com 9 agentes ativos em modo Foco, apenas 1 emite eventos `pty:data`.
Depende de F03-03, F03-04, F01-07.

### [ ] F03-06 — Controles da equipe
▶ Iniciar equipe (sobe os `autostart` em ordem, com escalonamento de 300 ms entre spawns),
⏸ Parar tudo, ⟳ Reiniciar tudo, com confirmação e feedback de progresso.
**Aceite:** iniciar uma equipe de 6 agentes não congela a UI em momento algum. Depende de F02-06.

### [ ] F03-07 — Sidebar de agentes
Lista com estado ao vivo, badge de mensagens pendentes (preparado para a Fase 5), reordenação por
arrastar, clique foca o painel, duplo clique abre o inspetor.
**Aceite:** a lista reflete mudanças de estado em <200 ms. Depende de F03-01.

### [ ] F03-08 — Navegação por teclado
`⌘1..9`, `⌘G` (ciclar vistas), `⌘T`, `⌘W`, `⌘\`, `⌘B`, `Esc Esc` para sair do foco do terminal.
Camada de atalhos global respeitando o foco do xterm.
**Aceite:** com o terminal focado, `⌘1` funciona e não vaza a tecla para o shell. Depende de F03-03.

### [ ] F03-09 — Inspetor do agente (T6)
Abas Visão, Config e Logs (as abas Skills e Caixa entram nas fases 4 e 5).
**Aceite:** a aba Logs mostra a transcrição com busca e exportação. Depende de F03-02.

## Critérios de saída
- [ ] 9 terminais simultâneos com performance dentro do orçamento de [03 — Stack](../03-stack.md#orçamento-de-performance-metas-verificáveis-na-fase-8)
- [ ] Estado de cada agente visível e correto
- [ ] Layout persistido por equipe
- [ ] Navegação inteiramente por teclado
- [ ] Trocar de vista não recria processos

## Riscos
| Risco | Mitigação |
|---|---|
| Detecção de estado imprecisa | Nesta fase o estado é só informativo (nada depende dele ainda); a precisão crítica só é exigida na Fase 5 |
| Grid arrastável consumir muito CPU | Usar transform em GPU; evitar reflow; virtualizar o que estiver fora da viewport |
