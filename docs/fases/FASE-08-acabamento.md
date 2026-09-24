# FASE 08 — Acabamento

**Objetivo:** transformar "funciona" em "é agradável de usar o dia inteiro".

**Demonstração:** usar o app por um dia inteiro de trabalho real sem incômodo — rápido, bonito,
previsível, acessível.

**Leitura obrigatória:** [08 — Design System](../08-design-system.md) ·
[03 — Stack](../03-stack.md#orçamento-de-performance-metas-verificáveis-na-fase-8) ·
[09 — Telas](../09-telas-e-fluxos.md)

## Tarefas

### [~] F08-01 — Auditoria de performance
Medir todas as métricas do orçamento de [03 — Stack](../03-stack.md#orçamento-de-performance-metas-verificáveis-na-fase-8)
com 12 agentes ativos. Perfilar e corrigir o que estourar.
**Aceite:** todas as métricas dentro da meta, com os números registrados em `docs/ESTADO.md`.

> Parcial. `scripts/perf-audit.py` sobe o **app real** (release empacotado, WebKitGTK) sob Xvfb
> com um diretório de dados novo, semeia uma equipe e usa "religar os agentes" (F08-06) para
> subir N agentes sem clique; mede cold start até a Sala na tela (log `tela da equipe`), PSS e CPU
> por processo. Os números estão em `docs/ESTADO.md` — foram medidos **sem GPU** (Mesa llvmpipe),
> o pior caso. **Corrigido pelo que a medição achou:** os pontos de estado animados ("iniciando",
> "trabalhando", "aguardando") repintavam a 60 fps e custavam um núcleo inteiro com 6 agentes;
> agora animam em passos (4–6 repinturas/s): 88% → 17% de CPU no mesmo cenário. A CSP do pacote
> bloqueava as fontes que o Vite embute como `data:` (`font-src`), e o terminal media os caracteres
> com `var(--font-mono)`, que o canvas não resolve — ambos corrigidos (F08-09). **Fora da meta
> aqui:** RAM (o processo da página do WebKit sozinho passa de 250 MB sem GPU; cada terminal em
> WebGL por software soma ~15–20 MB) e CPU com 12 agentes escrevendo sem parar (o xterm desenhando
> 240 linhas/s). **Não medido:** latência tecla→eco, `aisense send`→linha do tempo e reidratação
> do painel — precisam de instrumentação com o terminal real na tela. Falta medir numa máquina com
> GPU nos 3 SOs (macOS e Windows usam webviews nativos, sem este custo do WebKitGTK).

### [x] F08-02 — Auditoria de acessibilidade
Navegação completa por teclado, ordem de foco, rótulos ARIA, `aria-live` na timeline, contraste,
zoom de 80% a 150%, `prefers-reduced-motion`.
**Aceite:** percorrer os 5 fluxos críticos usando **apenas** o teclado, sem becos sem saída.

> Feito: `e2e/a11y.spec.ts` roda o axe-core (WCAG 2.1 A/AA; nova dependência de teste
> `@axe-core/playwright`) no onboarding, na lista de equipes, nas cinco vistas da Sala, em cada
> seção das Configurações e na biblioteca de skills, nos dois temas — zero violações. O que ele
> achou e foi corrigido: texto `muted` sem contraste sobre o fundo de item selecionado (sidebar e
> cabeçalho do painel), a alça de arrastar do painel envolvendo os botões dele (controles
> aninhados — agora só a identificação arrasta), e a linha do tempo e as colunas do quadro
> roláveis sem foco de teclado. `e2e/keyboard.spec.ts` percorre só com teclas: F1 (onboarding até
> a equipe rodando), F5 (`⌘⇧D` com 9 terminais, sem reload), F4 na interface (agente caído volta
> pelo menu do painel, com `⌘1` e `Esc Esc`), a paleta para qualquer tela, e Tab dando a volta nas
> telas sem prender o foco. F2 e F3 não têm passo de interface além do que a paleta cobre (o
> agente roda `aisense ask`; a skill é atribuída na aba Skills) — a parte deles é do core, com
> testes de processo real em Rust. **Zoom:** com 150% numa janela de 1024 px sobram 683 px de CSS;
> sidebar e inspetor agora cedem espaço sozinhos (`fitPanels`: o inspetor primeiro, a preferência
> do usuário não muda) e a barra da Sala quebra linha em vez de cortar controles. Movimento: a
> regra global de `prefers-reduced-motion` já existia; os esqueletos novos também param. Leitor de
> tela de verdade (VoiceOver/NVDA) não foi usado.

### [x] F08-03 — Estados vazios, carregamento e erro
Todo estado vazio com ilustração e ação; skeletons no lugar de spinners; toda mensagem de erro com
causa e próximo passo acionável.
**Aceite:** nenhuma tela mostra "algo deu errado" sem dizer o que fazer.

> Feito: `Skeleton`/`SkeletonList` e `ErrorNotice` no design system (`components/ui/`). Os
> "Carregando…"/"Verificando…" viraram esqueletos na forma do conteúdo (lista de equipes,
> biblioteca de skills, runtimes, assistente e formulário do agente, configurações, painéis da
> Sala antes da primeira lista — antes ela mostrava "Nenhum agente ainda" por um instante). Erros
> de carga usam `ErrorNotice`: a causa e a dica vêm do `CommandError` do core, com "Tentar de
> novo" quando faz sentido, e sem isso o caminho do diagnóstico (Configurações → Avançado). Vazios
> ganharam ação: equipe sem agentes tem o botão "Novo agente" (e `⌘T`); sidebar, canais e a lista
> sem equipe dizem o que fazer. Coberto por `e2e/states.spec.ts` (falha simulada no core falso,
> tentar de novo, vazios). Ilustrações continuam os ícones do `EmptyState` — não há ilustrações
> próprias no design system.

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

### [x] F08-07 — Notificações do sistema
Notificação do SO quando um agente entra em `awaiting_input` ou `failed` com o app em segundo plano;
ícone na bandeja com resumo. Configurável e silenciável.
**Aceite:** notificações não disparam quando a janela está focada na equipe em questão.

> Feito: a regra é pura, em `aisense-core/src/notify.rs` (`decide`): só `awaiting_input` e
> `failed`, cada um com o próprio interruptor, nada com as notificações desligadas ou silenciadas,
> e **nunca** para a equipe na tela com a janela em foco — o aceite é o teste
> `never_for_the_team_on_screen_with_the_window_focused`. Ela roda no core (`commands/notify.rs`),
> não no front: com a janela em segundo plano a webview pode ser suspensa, e o aviso existe
> justamente para esse caso. O front só informa qual equipe está na tela (`ui_viewing`); o foco
> da janela vem do Tauri. Bandeja: linha de resumo ("3 agentes rodando · 1 esperando você"),
> Mostrar o AISENSE, Silenciar por 1 hora / Reativar e Sair; sem bandeja no SO, o app sobe sem
> ela. Configurações → Notificações liga/desliga cada tipo e silencia por 1 h ou 8 h. Novas
> dependências: `tauri-plugin-notification` e a feature `tray-icon` do `tauri` (no Linux usa o
> `libayatana-appindicator3`, que o CI já instala). Falta ver o aviso e o ícone numa área de
> trabalho de verdade.

### [~] F08-08 — Testes E2E
Playwright cobrindo os 5 fluxos críticos de [09 — Telas](../09-telas-e-fluxos.md#fluxos-críticos-e2e-da-fase-8).
**Aceite:** a suíte roda no CI dos 3 SOs de forma estável (sem flake) por 10 execuções seguidas.

> Parcial. **Como é:** `apps/desktop/e2e/` roda o front de verdade no Chromium com o core em Rust
> trocado por um core falso em TypeScript (`src/e2e/fakeCore.ts`, via `mockIPC` do
> `@tauri-apps/api/mocks`, só no build `--mode e2e`). O teste semeia o estado
> (`window.__fakeSeed`) e dirige o core pelo `window.__fake` (derrubar um agente, rotear uma
> mensagem, fazer um comando falhar). `pnpm e2e` sobe um build de produção com o core falso e roda
> a suíte; o job `e2e` do CI faz o mesmo nos 3 SOs, **sem novas tentativas**. Nova dependência de
> teste: `@playwright/test`.
>
> **Os 5 fluxos** — o que a interface prova aqui e o que o core prova nos testes de processo
> real em Rust:
>
> | Fluxo | Interface (`e2e/`) | Core (Rust, processos reais) |
> |---|---|---|
> | F1 | `flows.spec.ts` F1: onboarding → Squad completo → ▶ → 4 terminais ociosos; e só com teclado em `keyboard.spec.ts` | `supervisor::tests` (start, `team_start` escalonado) |
> | F2 | `flows.spec.ts` F2: `ask` e `reply` entram ao vivo na linha do tempo | `aisense-cli/tests/e2e.rs` (agente num PTY roda `aisense send` e a mensagem chega), `bus::tests` (`pergunta_bloqueia_ate_a_resposta_e_destrava`, `resposta_volta_para_quem_perguntou`, deadlock, timeout) |
> | F3 | — (a parte visível é a aba Skills, sem fluxo novo) | `supervisor::tests::start_reports_the_skills_it_took_and_the_ones_it_ignored`, `skill::boot` (snapshot do `BOOT.md`), testes de entrega do boot (`boot_pela_flag…`, `boot_pelo_terminal…`) |
> | F4 | `flows.spec.ts` F4: cai, mostra "Erro" e volta sozinho; pelo teclado em `keyboard.spec.ts` | `supervisor::tests::killing_the_process_externally_restarts_on_crash`, políticas `never`/`on-crash`; `bus::tests::destinatario_parado_recebe_recado_mas_pergunta_e_recusada` (o recado espera na caixa) |
> | F5 | `flows.spec.ts` F5: 9 terminais, fundo do xterm muda, sem reload; `⌘⇧D` em `keyboard.spec.ts` | — |
>
> Além deles: `session.spec.ts` (F08-06), `states.spec.ts` (F08-03), `a11y.spec.ts` e
> `keyboard.spec.ts` (F08-02). **Falta** para o aceite: as 10 execuções seguidas no CI dos 3 SOs
> (aqui, em Linux, a suíte passou 10 vezes seguidas — ver `docs/ESTADO.md`), um teste de F4 que
> junte a queda com a entrega das mensagens pendentes depois da volta, e rodar os fluxos contra o
> app empacotado com o core real (WebDriver do Tauri), que ainda não existe.

### [x] F08-09 — Polimento visual final
Revisão painel a painel nos dois temas: alinhamentos, espaçamentos, pesos tipográficos, transições,
consistência de ícones. Comparação lado a lado claro/escuro de cada tela.
**Aceite:** screenshots de todas as telas nos dois temas anexados ao PR, sem inconsistência aberta.

> Feito: `pnpm --filter @aisense/desktop screenshots` gera as 20 telas nos dois temas com o core
> falso, em `docs/screenshots/fase-08/`. Lista fechada no início da revisão (timebox) e toda
> resolvida: (1) terminal com letras espaçadas — o xterm media com `var(--font-mono)`, que o canvas
> não entende, e não remedia quando a fonte chegava; (2) barra da Sala quebrando linha com uma ação
> sozinha — ações agrupadas e em tamanho `sm`, como o resto da barra; (3) texto do terminal
> encostado na borda — respiro interno; (4) Configurações e Skills mostrando a sidebar e o inspetor
> da equipe aberta — essas telas agora ocupam a área toda; (5) radios e checkboxes no azul do
> navegador — cor de acento do tema; (6) fontes embutidas bloqueadas pela CSP do pacote. As
> screenshots vêm do Chromium com o core falso, não da janela do Tauri.

## Critérios de saída
- [ ] Orçamento de performance cumprido e registrado — registrado; RAM e CPU com 12 ativos fora da meta sem GPU
- [x] Acessibilidade AA em todas as telas (axe, dois temas)
- [ ] Onboarding leva a uma equipe rodando em <2 min — 4 cliques no E2E; falta cronometrar com gente
- [ ] E2E estável nos 3 SOs — 10 execuções seguidas em Linux; falta o CI dos 3 SOs
- [x] Nenhum estado vazio, de carregamento ou de erro sem tratamento

## Riscos
| Risco | Mitigação |
|---|---|
| E2E com terminais reais ser instável | Usar um runtime `shell` determinístico nos testes, nunca uma CLI de IA real |
| Polimento virar buraco sem fundo | Timebox: a lista de F08-09 é fechada no início da fase e não cresce durante ela |
