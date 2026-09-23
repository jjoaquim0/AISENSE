# ADR 0007 — Atalhos com o terminal focado

- **Status:** aceito
- **Data:** 2026-09-23
- **Fase:** 03 (F03-08)

## Contexto
O `docs/08` define atalhos com `⌘`, que no Linux e no Windows vira `Ctrl`. Com um terminal
focado, a tecla vai para o textarea escondido do xterm, que a consome e manda ao processo.
Dois problemas opostos: se o app não interceptar, `⌘1` nunca troca de painel com o terminal
focado (e o aceite da F03-08 exige que troque); se interceptar tudo, `Ctrl+W` (apagar palavra),
`Ctrl+B` (prefixo do tmux), `Ctrl+\` (SIGQUIT), `Ctrl+G`, `Ctrl+T` e `Ctrl+K` deixam de chegar
ao shell — e um terminal que engole essas teclas é um terminal quebrado.

## Opções consideradas
| Opção | Prós | Contras |
|---|---|---|
| Interceptar todo atalho, em todo SO | Um comportamento só | Quebra o shell no Linux/Windows |
| Nunca interceptar com o terminal focado | Shell intacto | `⌘1` não funciona de dentro do terminal (aceite da F03-08) |
| Usar `Ctrl+Shift` no Linux/Windows | Convenção de emuladores de terminal | Muda a tabela do doc 08 por SO; mais para aprender |
| **Interceptar por atalho: `⌘1..9` sempre; o resto só no macOS; `Esc Esc` para sair** | Shell intacto, troca de painel sempre funciona | Fora do macOS, `⌘W`/`⌘G`/... pedem `Esc Esc` antes |

## Decisão
Uma camada de atalhos única, na fase de captura da janela (antes do xterm). Com o terminal
focado, só interceptamos os atalhos marcados `inTerminal: 'always'` — hoje, `⌘1..9`, porque
`Ctrl+dígito` não tem uso em shell — e, no macOS, todos (lá `⌘` nunca é tecla de shell).
`Esc Esc` (duas em 400 ms) tira o foco do terminal para o painel: a primeira `Esc` segue para o
processo, a segunda não.

## Consequências
**Positivas:** o shell recebe todas as suas teclas de controle; trocar de painel funciona de
qualquer lugar; um só listener para o app inteiro.
**Negativas / custos aceitos:** no Linux/Windows, `⌘W`, `⌘G`, `⌘T`, `⌘\`, `⌘B`, `⌘I` exigem
sair do terminal antes (`Esc Esc`). Diálogos abertos não recebem atalhos globais.
**O que passa a ser proibido:** marcar como `'always'` um atalho cuja tecla com `Ctrl` tenha
significado no shell; registrar `keydown` global fora de `useShortcuts`.

## Quando revisitar
Na tela de remapeamento de atalhos (Fase 08): se usuários de Linux/Windows pedirem, oferecer a
variante `Ctrl+Shift` como opção.
