# FASE 08 — Acabamento

**Objetivo:** transformar "funciona" em "é agradável de usar o dia inteiro".

**Demonstração:** usar o app por um dia inteiro de trabalho real sem incômodo — rápido, bonito,
previsível, acessível.

**Leitura obrigatória:** [08 — Design System](../08-design-system.md) ·
[03 — Stack](../03-stack.md#orçamento-de-performance-metas-verificáveis-na-fase-8) ·
[09 — Telas](../09-telas-e-fluxos.md)

## Tarefas

### [ ] F08-01 — Auditoria de performance
Medir todas as métricas do orçamento de [03 — Stack](../03-stack.md#orçamento-de-performance-metas-verificáveis-na-fase-8)
com 12 agentes ativos. Perfilar e corrigir o que estourar.
**Aceite:** todas as métricas dentro da meta, com os números registrados em `docs/ESTADO.md`.

### [ ] F08-02 — Auditoria de acessibilidade
Navegação completa por teclado, ordem de foco, rótulos ARIA, `aria-live` na timeline, contraste,
zoom de 80% a 150%, `prefers-reduced-motion`.
**Aceite:** percorrer os 5 fluxos críticos usando **apenas** o teclado, sem becos sem saída.

### [ ] F08-03 — Estados vazios, carregamento e erro
Todo estado vazio com ilustração e ação; skeletons no lugar de spinners; toda mensagem de erro com
causa e próximo passo acionável.
**Aceite:** nenhuma tela mostra "algo deu errado" sem dizer o que fazer.

### [x] F08-04 — Onboarding (T1)
Fluxo de 3 passos com detecção de runtimes, escolha de tema e criação da primeira equipe.
**Aceite:** usuário novo sai do onboarding com uma equipe rodando em menos de 2 minutos.

> Feito: `apps/desktop/src/features/onboarding/`. Aparece enquanto `settings.onboardingDone` for
> `false` (F08-05); pular em qualquer passo ou criar a equipe grava `true` e ele nunca mais volta.
> Passo 1: runtimes detectados (a `RuntimeList` da T2, com dica de instalação); passo 2: tema com
> efeito imediato; passo 3: nome (já preenchido), pasta, modelo (Dupla Dev por padrão, ou Squad
> completo, ou Vazio) e "Iniciar os agentes ao criar" marcado. Agente cujo runtime não está
> instalado vai para o **shell** (`withRunnableRuntimes`), com o aviso de qual trocou — a primeira
> equipe sempre sobe. Criar abre a Sala da Equipe e dispara o ▶. Sem janela aqui, os dois
> minutos foram conferidos só no E2E com o core falso (F08-08): o caminho é Continuar, Continuar,
> escolher a pasta, Criar — quatro cliques. Falta cronometrar com um usuário novo de verdade.

### [x] F08-05 — Configurações (T9)
Todas as seções de [09 — Telas](../09-telas-e-fluxos.md#t9--configurações), incluindo o modo
calibração de estado e o gerenciamento de segredos via keychain.
**Aceite:** ajustar `idle_regex` no modo calibração reflete no detector sem reiniciar o app.

> Feito: preferências em `aisense-core/src/settings.rs` (`AppSettings`, gravado em
> `~/.aisense/settings.json` com escrita atômica; arquivo ilegível vira os padrões e é guardado
> como `settings.json.corrupt`; valores fora da faixa são trazidos para dentro). Comandos em
> `aisense-app/src/commands/settings.rs`; tela em `apps/desktop/src/features/settings/`
> (⚙ no trilho, `⌘,`, paleta). **Calibração:** `StateDetector::set_rules` troca as regras de uma
> sessão viva; `AgentSupervisor::refresh_state_rules` manda as do catálogo atual para todas —
> chamado pelo `calibration_apply` e pelo hot-reload dos adaptadores. `state::calibrate` diz o que
> o detector decidiria para uma tela, com a linha e o erro de cada regex; `adapter::save_state_rules`
> grava no arquivo do usuário ou numa cópia do embutido em `~/.aisense/adapters/`, validando antes
> de escrever. Aceite coberto por `new_state_rules_reach_a_live_session_without_restart` (processo
> real, mesmo PID, uma sessão só). **Barramento:** limites anti-laço, timeout do `ask` e retenção
> saíram de constantes e valem sem reiniciar (`BusService::set_limits`). **Segredos:** crate
> `keyring` (nova dependência, justificada em `docs/11`); o valor vai ao keychain, o
> `settings.json` guarda só runtime, variável e a forma `sk-…abcd`; o supervisor põe no ambiente
> do agente no start, sem sobrescrever o `env` do próprio agente. **Atalhos:** remapeamento na
> camada única (`buildRemap`/`resolveCombo`); ⌘1..9 e Esc Esc ficam fixos. Aparência: tema,
> densidade (altura de linha do terminal), fonte e tamanho do terminal ao vivo. Avançado: nível
> de log (na próxima subida), exportar diagnóstico (sem segredos), resetar preferências.
> Fora: "instalar integração MCP" (o AISENSE já registra o `aisense-mcp` no start onde o
> adaptador tem `mcp_config`, F05-09) e o editor de TOML dentro do app (os adaptadores são
> editados na pasta, com hot-reload).

### [x] F08-06 — Persistência de sessão
Restaurar equipe aberta, vista, layout, painel focado e tamanhos ao reabrir o app.
Opção "religar agentes ao abrir".
**Aceite:** fechar com 6 agentes e reabrir restaura tudo, inclusive o scroll da timeline.

> Feito: a equipe aberta vai para `settings.session.lastTeam` a cada troca e é reaberta na subida
> (`features/session/useSessionRestore.ts`; apagada ou arquivada, não). Vista, grade e posições do
> Fluxo já estavam em `teams.layout`; agora também o **agente em foco** (`focused`) e a **rolagem
> da linha do tempo** (`timeline: { anchor, delta }` — a mensagem no topo da tela e quanto dela já
> passou, ou nada quando se está no fim acompanhando as novas; a âncora sobrevive a mensagens
> novas chegando). Larguras de sidebar e inspetor continuam no `usePanels`. **Religar agentes:** ao
> fechar a janela o app grava quem estava de pé (`AgentSupervisor::running_agents` →
> `session.runningAgents`); na subida, com a opção ligada, sobe cada um escalonado como o ▶ da
> equipe. Aceite coberto por `e2e/session.spec.ts` (6 agentes, 400 mensagens, reload no lugar de
> fechar o app, 5 execuções seguidas sem falha) — com o core falso: a gravação do layout pelo
> core real é a mesma de antes (`team_set_layout`).

### [ ] F08-07 — Notificações do sistema
Notificação do SO quando um agente entra em `awaiting_input` ou `failed` com o app em segundo plano;
ícone na bandeja com resumo. Configurável e silenciável.
**Aceite:** notificações não disparam quando a janela está focada na equipe em questão.

### [ ] F08-08 — Testes E2E
Playwright cobrindo os 5 fluxos críticos de [09 — Telas](../09-telas-e-fluxos.md#fluxos-críticos-e2e-da-fase-8).
**Aceite:** a suíte roda no CI dos 3 SOs de forma estável (sem flake) por 10 execuções seguidas.

### [ ] F08-09 — Polimento visual final
Revisão painel a painel nos dois temas: alinhamentos, espaçamentos, pesos tipográficos, transições,
consistência de ícones. Comparação lado a lado claro/escuro de cada tela.
**Aceite:** screenshots de todas as telas nos dois temas anexados ao PR, sem inconsistência aberta.

## Critérios de saída
- [ ] Orçamento de performance cumprido e registrado
- [ ] Acessibilidade AA em todas as telas
- [ ] Onboarding leva a uma equipe rodando em <2 min
- [ ] E2E estável nos 3 SOs
- [ ] Nenhum estado vazio, de carregamento ou de erro sem tratamento

## Riscos
| Risco | Mitigação |
|---|---|
| E2E com terminais reais ser instável | Usar um runtime `shell` determinístico nos testes, nunca uma CLI de IA real |
| Polimento virar buraco sem fundo | Timebox: a lista de F08-09 é fechada no início da fase e não cresce durante ela |
