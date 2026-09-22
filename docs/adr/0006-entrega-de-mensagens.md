# ADR 0006 — Entrega híbrida: caixa de entrada, injeção e hook

- **Status:** aceito
- **Data:** 2026-09-22
- **Fase:** 00

## Contexto

Esta é a decisão mais delicada do projeto. Uma mensagem chegou para `@frontend`.
Como ela entra na cabeça de uma IA que está rodando dentro de um terminal interativo?

O AISENSE **não controla** o runtime: não sabe onde está o cursor, se a IA está no meio de uma
resposta, se está aguardando confirmação de um comando destrutivo. Só enxerga bytes saindo do PTY.

Escrever no stdin no momento errado causa estrago real: um `\n` no meio de um diálogo de confirmação
pode aprovar `rm -rf`. Não é hipótese, é o comportamento normal de um terminal.

## Opções consideradas

| Opção | Prós | Contras |
|---|---|---|
| Sempre injetar no stdin | Reativo, parece mágica | Perigoso: depende de heurística para saber quando é seguro |
| Só caixa de entrada (pull) | 100% seguro | Depende de a IA lembrar de checar; mensagem pode ficar parada por muito tempo |
| Só hook do runtime | Melhor dos mundos onde existe | Só Claude Code (e poucos) suportam |
| **Híbrido, configurável por agente** | Cada agente usa o melhor caminho disponível | Três caminhos para manter e testar |

## Decisão

**Três modos, escolhidos por agente**, com `pull` como padrão:

1. `pull` — mensagem fica na caixa; a IA lê com `aisense inbox`. **Padrão.** Risco zero.
2. `push` — injeção no stdin, **somente** sob condições estritas (ver abaixo).
3. `hook` — o runtime checa a caixa sozinho ao fim de cada turno. **Recomendado onde existir.**

### Condições obrigatórias para injetar (modo `push`)

Todas precisam valer:
1. Estado detectado = `idle` com confiança alta (silêncio ≥ `quiet_ms` **e** `idle_regex` casando).
2. Estado **não** é `awaiting_input` — se a IA está pedindo confirmação, nunca injetar.
3. Passou o throttle (≥ 3 s desde a última injeção nesse agente).
4. Corpo sanitizado (sem ESC, sem `\r`, sem C0) e dentro de `inject.max_chars`.
5. O `\r` de submissão é adicionado pelo AISENSE, jamais vem do corpo.

Se a confiança for baixa, a mensagem **rebaixa para `pull`** naquela rodada, em vez de arriscar.

## Consequências

**Positivas**
- O padrão é seguro: quem não configura nada não corre risco.
- Onde há hook, a experiência é ótima sem depender de heurística.
- `push` continua disponível para quem quer agentes reativos e aceita o trade-off.

**Negativas / custos aceitos**
- Três caminhos de entrega para testar. Mitigado por testes de integração por modo.
- A heurística de estado vai errar em algum runtime. Mitigado por: regex configurável por adaptador,
  modo calibração na UI, rebaixamento automático e o fato de a injeção ser sempre visível na tela.

**O que passa a ser proibido**
- Injetar com estado desconhecido ou `awaiting_input`.
- Injetar corpo não sanitizado.
- Injeção silenciosa: toda injeção aparece marcada na UI e na linha do tempo.

## Quando revisitar

Quando houver um mecanismo padronizado de "entrada assíncrona" nos runtimes (algo como um endpoint
de mensagem no próprio agente). Aí `push` vira legado e `hook` generalizado vira o padrão.
