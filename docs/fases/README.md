# Fases de Desenvolvimento

> O projeto é dividido em 10 fases. Cada fase entrega **algo demonstrável** — nada de fase que só
> produz abstração. A regra é simples: ao fim de cada fase, dá para abrir o app e mostrar para alguém.

## Mapa

```
FASE 0 ─ Fundação
   │     (monorepo, Tauri roda, design tokens, CI)
   ▼
FASE 1 ─ Terminal Core
   │     ★ "um terminal de verdade dentro do app"
   ▼
FASE 2 ─ Equipes e Agentes
   │     ★ "criei uma equipe e 3 agentes, e eles persistem"
   ▼
FASE 3 ─ Sala da Equipe ──────┐
   │     ★ "4 terminais lado a lado, todos vivos"
   │                          │
   ▼                          ▼
FASE 4 ─ Skills           (4 e 3 podem ser paralelas após a 2)
   │     ★ "o agente nasce já sabendo quem é"
   ▼
FASE 5 ─ Barramento
   │     ★ "os agentes conversam entre si"   ← o coração do produto
   ▼
FASE 6 ─ Quadro Kanban
   │     ★ "o agente puxa trabalho do quadro sozinho"
   ▼
FASE 7 ─ Coordenação
   │     ★ "o coordenador distribui e o canvas mostra o fluxo"
   ▼
FASE 8 ─ Acabamento
   │     ★ "está bonito, rápido e acessível"
   ▼
FASE 9 ─ Distribuição
         ★ "tem instalador nos 3 SOs"
```


## Marco de fatiamento vertical

As Fases 5 e 6 são o coração: sem elas isto é só um gerenciador de terminais bonito.
Se for preciso cortar escopo, corte **largura**, nunca profundidade — é melhor ter 2 runtimes com
barramento e quadro completos do que 6 runtimes que não conversam e não compartilham estado.

## Formato de cada fase

Cada documento tem:
- **Objetivo** — uma frase
- **Demonstração** — o que dá para mostrar no fim
- **Leitura obrigatória** — quais docs ler antes
- **Tarefas** — com ID (`FXX-NN`), dependências e critério de aceite
- **Critérios de saída** — checklist para declarar a fase concluída
- **Riscos**

## Convenção de status das tarefas

```
[ ]  não iniciada
[~]  em andamento ou parcial (SEMPRE com uma nota do que falta)
[x]  concluída, atendendo a Definition of Done de docs/10-padroes-de-codigo.md
[!]  bloqueada (SEMPRE com uma nota do motivo)
```

## Estimativa de esforço

Estimativas em **sessões de agente** (uma sessão ≈ uma tarefa média com testes), não em dias.
Servem para sequenciar, não para prometer prazo.

| Fase | Tarefas | Esforço relativo |
|---|---|---|
| 0 | 9 | ▰▰▰ |
| 1 | 8 | ▰▰▰▰ |
| 2 | 9 | ▰▰▰ |
| 3 | 9 | ▰▰▰▰ |
| 4 | 8 | ▰▰▰ |
| 5 | 11 | ▰▰▰▰▰▰ |
| 6 | 10 | ▰▰▰▰▰ |
| 7 | 6 | ▰▰▰ |
| 8 | 9 | ▰▰▰▰ |
| 9 | 7 | ▰▰ |

**Total: 86 tarefas.**
