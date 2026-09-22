# 08 — Design System

> Objetivo estético: **uma ferramenta profissional que dá vontade de deixar aberta o dia inteiro.**
> Referências de linguagem visual: Linear (densidade e calma), Raycast (velocidade e paleta de comandos),
> Warp (terminal como cidadão de primeira classe), Zed (tipografia).
> Princípio central: **a interface é o palco, o terminal é o ator.** O cromo nunca compete com o conteúdo.

## Tipografia

| Papel | Fonte | Por quê |
|---|---|---|
| Interface | **Inter Variable** | Altura-x generosa, excelente em 12–14px, variável (um arquivo, todos os pesos) |
| Terminal / código | **JetBrains Mono** | Desenhada para leitura longa de código, ligaduras opcionais, ótima distinção `0/O`, `1/l/I` |
| Números tabulares | Inter com `font-variant-numeric: tabular-nums` | Métricas não "dançam" ao atualizar |

Fontes são **empacotadas no app** (WOFF2 subset latino), nunca carregadas de CDN — o app precisa
funcionar offline e sem vazar requisição nenhuma.

```css
--font-sans: "Inter Variable", system-ui, -apple-system, "Segoe UI", sans-serif;
--font-mono: "JetBrains Mono", ui-monospace, "SF Mono", Consolas, monospace;
```

### Escala tipográfica

| Token | Tamanho / Entrelinha | Peso | Uso |
|---|---|---|---|
| `text-display` | 28 / 34 px | 600 | Título de tela vazia, onboarding |
| `text-title` | 20 / 28 px | 600 | Nome da equipe no cabeçalho |
| `text-heading` | 15 / 22 px | 600 | Títulos de seção, nome do agente no painel |
| `text-body` | 13,5 / 20 px | 400 | Texto padrão da interface |
| `text-label` | 12 / 16 px | 500 | Rótulos de campo, abas |
| `text-caption` | 11 / 14 px | 500 | Metadados, timestamps, contadores |
| `text-mono` | 13 / 20 px | 400 | Terminal (ajustável pelo usuário: 11–18px) |

Letter-spacing: `-0.011em` em `display`/`title` (Inter fica mais compacta e elegante em tamanhos grandes),
`0` no resto, `0.02em` em `caption` maiúsculo.

## Cor

Toda cor é definida em **OKLCH** — perceptualmente uniforme, o que significa que o mesmo `L` parece
igualmente claro em qualquer matiz. Isso é o que permite a paleta de agentes ficar harmônica.

### Tokens primitivos (não use direto em componente)

```css
/* Neutros — leve viés azulado (chroma 0.008–0.02) para não parecer cinza morto */
--gray-0:  oklch(0.99 0.002 265);   --gray-50:  oklch(0.97 0.004 265);
--gray-100:oklch(0.94 0.006 265);   --gray-200: oklch(0.89 0.008 265);
--gray-300:oklch(0.80 0.010 265);   --gray-400: oklch(0.66 0.012 265);
--gray-500:oklch(0.54 0.014 265);   --gray-600: oklch(0.44 0.014 265);
--gray-700:oklch(0.33 0.014 265);   --gray-800: oklch(0.25 0.014 265);
--gray-850:oklch(0.21 0.014 265);   --gray-900: oklch(0.17 0.014 265);
--gray-950:oklch(0.13 0.012 265);

/* Marca */
--brand-400: oklch(0.72 0.16 285);  --brand-500: oklch(0.63 0.19 285);
--brand-600: oklch(0.55 0.20 285);  --brand-700: oklch(0.47 0.18 285);

/* Semânticas de estado */
--green-500: oklch(0.70 0.17 150);  /* ocioso / sucesso */
--amber-500: oklch(0.78 0.16  75);  /* aguardando input / atenção */
--blue-500:  oklch(0.65 0.17 240);  /* ocupado / informação */
--red-500:   oklch(0.62 0.21  25);  /* erro / falha */
```

### Tokens semânticos (use SEMPRE estes)

| Token | Claro | Escuro | Uso |
|---|---|---|---|
| `--bg-base` | `gray-50` | `gray-950` | Fundo da janela |
| `--bg-surface` | `gray-0` | `gray-900` | Cards, painéis, sidebar |
| `--bg-raised` | `gray-0` | `gray-850` | Popover, modal, dropdown |
| `--bg-terminal` | `oklch(0.99 0 0)` | `oklch(0.15 0.010 265)` | Fundo do xterm |
| `--bg-hover` | `gray-100` | `gray-800` | Hover de item de lista |
| `--bg-active` | `gray-200` | `gray-700` | Item selecionado |
| `--fg-primary` | `gray-900` | `gray-50` | Texto principal |
| `--fg-secondary` | `gray-600` | `gray-300` | Texto de apoio |
| `--fg-muted` | `gray-500` | `gray-400` | Metadados, placeholder |
| `--border-subtle` | `gray-200` | `gray-800` | Divisórias |
| `--border-strong` | `gray-400` | `gray-500` | Contorno de input (3:1, WCAG 1.4.11) |
| `--accent` | `brand-600` | `brand-400` | Ação primária |
| `--accent-fg` | `gray-0` | `gray-950` | Texto sobre accent |
| `--ring` | `brand-500` | `brand-400` | Anel de foco |

**Contraste:** todo par texto/fundo é validado em AA (4.5:1 para corpo, 3:1 para ≥18px e para
elementos de interface não textuais — indicadores de estado, anel de foco e contorno de campo,
conforme WCAG 1.4.11). O teste `apps/desktop/src/styles/__tests__/contrast.test.ts` verifica 54
pares nos dois temas e falha o CI abaixo do mínimo. Isso não é opcional: já reprovou
`--fg-muted` (4,45:1) e `--border-strong` (1,8:1) na primeira execução, e os tokens é que mudaram.

O teste também garante que as 8 cores de agente têm **peso visual parecido** (variação de contraste
menor que 2×): com 9 terminais na tela, uma cor muito mais pesada que as outras desequilibra o mosaico.

### Paleta de identidade dos agentes

Com 9 terminais na tela, **cor é a principal ajuda de navegação**. Cada agente recebe uma cor usada
na borda do painel, na bolha da linha do tempo, no nó do canvas e no avatar. Oito matizes com
`L` e `C` constantes, garantindo peso visual idêntico:

```css
--agent-violet: oklch(0.65 0.18 285);   --agent-cyan:   oklch(0.65 0.18 205);
--agent-emerald:oklch(0.65 0.18 155);   --agent-amber:  oklch(0.72 0.16  75);
--agent-rose:   oklch(0.65 0.18  15);   --agent-indigo: oklch(0.65 0.18 265);
--agent-teal:   oklch(0.65 0.18 180);   --agent-fuchsia:oklch(0.65 0.18 320);
```

No tema claro, sobe `L` para `0.52` (contraste com fundo claro). Atribuição automática em ordem ao
criar agentes, com troca manual. As cores são distinguíveis em deuteranopia e protanopia — validado
com simulador; mesmo assim, **cor nunca é o único sinal**: estado sempre tem ícone e texto junto.

### Tema do terminal

O xterm recebe um tema derivado dos mesmos tokens, com as 16 cores ANSI ajustadas para contraste
legível nos dois temas (a paleta padrão do xterm tem azul ilegível em fundo escuro — corrigir).
Tabela completa em `apps/desktop/src/features/terminal/theme.ts`. O usuário pode escolher entre
`AISENSE Dark`, `AISENSE Light` e importar temas no formato do Windows Terminal / iTerm2.

## Alternância de tema

Três estados: `claro` · `escuro` · `sistema` (padrão). Implementação:

```css
:root { /* tokens do tema claro */ }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { /* tokens do escuro */ }
}
:root[data-theme="dark"] { /* tokens do escuro */ }
```

A classe é aplicada no `<html>` **antes da primeira pintura** (script inline no `index.html` lendo
`localStorage`) para não haver flash branco. Transição de 150 ms em `background-color` e `color`,
desligada quando `prefers-reduced-motion`.

## Espaçamento, raio e elevação

Escala de 4 px: `0 · 4 · 8 · 12 · 16 · 24 · 32 · 48 · 64`. Densidade de aplicação profissional:
padding de item de lista `6px 10px`, altura de linha de lista `32px`, altura de botão `32px`
(`28px` na variante compacta).

```css
--radius-sm: 6px;    /* badge, chip */
--radius-md: 8px;    /* botão, input */
--radius-lg: 12px;   /* card, painel de terminal */
--radius-xl: 16px;   /* modal */
```

Elevação por **sombra sutil + borda**, nunca por sombra pesada. No tema escuro a sombra quase não
aparece — a separação vem da borda e de 1 passo de luminosidade no fundo:

```css
--shadow-sm: 0 1px 2px oklch(0 0 0 / 0.05);
--shadow-md: 0 4px 12px oklch(0 0 0 / 0.08), 0 1px 2px oklch(0 0 0 / 0.04);
--shadow-lg: 0 12px 32px oklch(0 0 0 / 0.12), 0 2px 6px oklch(0 0 0 / 0.06);
```

## Movimento

Rápido e discreto. Interface de trabalho não faz coreografia.

| Situação | Duração | Easing |
|---|---|---|
| Hover, foco | 100 ms | `ease-out` |
| Popover, dropdown, tooltip | 150 ms | `cubic-bezier(0.16, 1, 0.3, 1)` |
| Modal, painel lateral | 200 ms | `cubic-bezier(0.16, 1, 0.3, 1)` |
| Troca de layout do grid | 250 ms | `cubic-bezier(0.34, 1.2, 0.64, 1)` |
| Mensagem entrando na timeline | 180 ms fade + 4px slide | `ease-out` |

**`prefers-reduced-motion: reduce` zera todas as durações** — implementado uma vez no CSS global.

## Indicadores de estado do agente

Sempre **ponto colorido + ícone + texto**. Nunca só cor.

| Estado | Cor | Forma | Rótulo |
|---|---|---|---|
| `idle` | verde | ● sólido | Ocioso |
| `busy` | azul | ◐ com rotação lenta (2 s) | Trabalhando |
| `awaiting_input` | âmbar | ● pulsando (1,5 s) | Aguardando você |
| `failed` | vermelho | ▲ triângulo | Erro |
| `stopped` | cinza | ○ vazado | Parado |
| `starting` | azul | ○ com anel girando | Iniciando |

Quando há mensagem pendente, o badge de contagem aparece sobre o ponto, na cor do agente remetente.

## Componentes base (construir na Fase 0)

`Button` (primary/secondary/ghost/danger · sm/md) · `IconButton` · `Input` · `Textarea` ·
`Select` · `Switch` · `Checkbox` · `Tabs` · `Dialog` · `Sheet` · `DropdownMenu` · `Tooltip` ·
`Popover` · `Badge` · `Avatar` (inicial do handle na cor do agente) · `StatusDot` · `Kbd` ·
`Toast` · `EmptyState` · `Skeleton` · `Resizable` · `ScrollArea` · `CommandPalette` · `SplitPane`.

Base: Radix Primitives (acessibilidade e comportamento de teclado prontos) com estilo próprio via
Tailwind + `class-variance-authority`. **Não use biblioteca de componentes prontos com visual próprio** —
o visual é nosso.

## Acessibilidade (obrigatório, não "depois")

1. Todo elemento interativo é alcançável por `Tab`, com anel de foco visível de 2 px em `--ring`.
2. Nenhuma informação transmitida só por cor.
3. Contraste AA em tudo, verificado em CI.
4. `aria-live="polite"` na linha do tempo; mudanças de estado de agente são anunciadas.
5. O terminal expõe `role="application"` com instrução de como sair do foco (`Escape Escape`).
6. Alvo de clique mínimo de 28×28 px (a densidade é alta, mas não hostil).
7. Zoom da interface de 80% a 150% sem quebra de layout.

## Atalhos de teclado

| Atalho | Ação |
|---|---|
| `⌘K` / `Ctrl+K` | Paleta de comandos |
| `⌘1`–`⌘9` | Focar painel N |
| `⌘T` | Novo agente na equipe atual |
| `⌘⇧T` | Nova equipe |
| `⌘\` | Dividir painel |
| `⌘W` | Fechar painel focado |
| `⌘Enter` | Enviar mensagem do compositor |
| `⌘G` | Alternar modo de vista (Grid → Foco → Fluxo → Timeline) |
| `⌘B` | Mostrar/ocultar sidebar |
| `⌘I` | Mostrar/ocultar inspetor |
| `⌘⇧D` | Alternar tema claro/escuro |
| `⌘J` | Mostrar/ocultar linha do tempo |
| `⌘F` | Buscar no terminal focado |
| `⌘⇧F` | Buscar em todos os terminais e mensagens |
| `Esc Esc` | Sair do foco do terminal para a navegação da UI |

Todos remapeáveis em Configurações → Atalhos. No Linux/Windows, `⌘` vira `Ctrl`.
