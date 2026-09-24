# Guia — Skills

> Uma **skill** é um pacote de instruções que um agente carrega ao nascer: é o que faz o terminal
> subir sabendo o seu papel, sem você redigitar o mesmo prompt a cada vez. O formato é o mesmo das
> skills do Claude Code — uma skill que você já tem funciona sem conversão. Referência completa:
> [06 — Sistema de Skills](../06-sistema-de-skills.md).

## O que já vem pronto

| Skill | Para quê |
|---|---|
| `trabalho-em-equipe` | Ensina qualquer IA a usar o barramento (`aisense send`, `ask`, `inbox`...). Vai em **todo** agente, sem precisar atribuir |
| `coordenador` | Quebra o objetivo em cartões, distribui pelo papel de cada um e acompanha pelo quadro |
| `implementador` | Implementa um cartão pequeno de ponta a ponta, com testes, e avisa o revisor |
| `revisor-rigoroso` | Revisa diffs procurando defeitos de correção, não estilo |
| `pesquisador` | Investiga uma pergunta e publica achados verificáveis |
| `sintetizador` | Junta os achados de vários agentes num relatório, apontando conflitos |
| `documentador` | Mantém README, docs e notas da equipe em dia com o código |

As embutidas não se editam; **Duplicar para editar** cria uma cópia sua.

## Criar uma skill

**Pelo app:** barra lateral → **Biblioteca de skills** → **Nova skill**. O editor valida enquanto
você digita, mostra a prévia do Markdown e conta os caracteres do corpo. **Salvar** grava em
`~/.aisense/skills/<nome>/SKILL.md` (Windows: `%APPDATA%\AISENSE\skills\`).

**À mão:** crie a pasta e o arquivo — o app percebe sozinho.

```
~/.aisense/skills/
└── rust-idiomatico/
    ├── SKILL.md
    └── references/
        └── erros.md        # opcional: a IA lê sob demanda
```

```markdown
---
name: rust-idiomatico
description: Escreve Rust idiomático, com erros tipados. Use em qualquer mudança em .rs.
version: 1.0.0
targets: [claude, codex]    # ausente = serve para todos os runtimes
inject: bootstrap           # bootstrap | reference | mcp
priority: 50                # menor entra primeiro
---

# Rust idiomático

- Erros com `thiserror`; nada de `unwrap()` fora de teste.
- Consulte `references/erros.md` antes de criar um tipo de erro novo.
```

| Campo | Obrigatório | Dica |
|---|---|---|
| `name` | sim | igual ao nome da pasta; minúsculas e hífen |
| `description` | sim | uma frase dizendo **quando** usar — é o que a IA lê primeiro |
| `targets` | não | se o runtime do agente não estiver na lista, a skill é ignorada e o app avisa |
| `inject` | não | `bootstrap` entra no `BOOT.md`; `reference` só copia o arquivo e cita o caminho |
| `priority` | não | de 0 a 100 |

Um erro no frontmatter aparece no editor e na biblioteca com **arquivo e linha**, e a skill não
carrega até ser corrigida (botão **Consertar**).

## Dar a skill a um agente

1. Na Sala da Equipe, dê duplo clique no agente (ou use a barra lateral) para abrir o inspetor.
2. Aba **Skills** → **Adicionar skill**. As setas de cada linha mudam a ordem.
3. A lista **No próximo início** mostra exatamente o que vai valer e o que será ignorado (e por quê).

Skills **não** mudam um agente que já está rodando: trocar as instruções no meio de uma conversa
confunde a IA. Ao salvar uma skill em uso, o app lista quem precisa reiniciar e oferece reiniciar
esses agentes.

## Onde a skill vai parar

No início do agente, o AISENSE:

1. copia as skills ativas para `<pasta da equipe>/.aisense/agents/<handle>/skills/` — e, quando o
   runtime tem pasta própria (o Claude Code usa `.claude/skills/`), também para lá, sem mexer nas
   skills que já eram suas nessa pasta;
2. escreve `<pasta da equipe>/.aisense/agents/<handle>/BOOT.md` com a identidade do agente, a
   equipe e o corpo das skills `bootstrap`;
3. entrega o `BOOT.md` pelo melhor caminho que o runtime aceita (veja o
   [guia de adaptadores](adaptadores.md#4-mensagens-e-o-bootmd)).

O `BOOT.md` tem limite de 12.000 caracteres. Passando disso, as skills entram resumidas, com o caminho
do arquivo completo para a IA ler quando precisar, e o app avisa.

## Compartilhar

- **Exportar…** (menu da skill na biblioteca) copia a pasta da skill para onde você escolher.
- **Importar** traz uma pasta com `SKILL.md` para a sua biblioteca. O app ainda não mostra uma
  prévia antes de importar: abra o `SKILL.md` e leia antes — uma skill de terceiros é um texto que
  a IA vai seguir. Depois de importada, ela abre no editor como qualquer outra.
