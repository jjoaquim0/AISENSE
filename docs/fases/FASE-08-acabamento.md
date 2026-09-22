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

### [ ] F08-04 — Onboarding (T1)
Fluxo de 3 passos com detecção de runtimes, escolha de tema e criação da primeira equipe.
**Aceite:** usuário novo sai do onboarding com uma equipe rodando em menos de 2 minutos.

### [ ] F08-05 — Configurações (T9)
Todas as seções de [09 — Telas](../09-telas-e-fluxos.md#t9--configurações), incluindo o modo
calibração de estado e o gerenciamento de segredos via keychain.
**Aceite:** ajustar `idle_regex` no modo calibração reflete no detector sem reiniciar o app.

### [ ] F08-06 — Persistência de sessão
Restaurar equipe aberta, vista, layout, painel focado e tamanhos ao reabrir o app.
Opção "religar agentes ao abrir".
**Aceite:** fechar com 6 agentes e reabrir restaura tudo, inclusive o scroll da timeline.

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
