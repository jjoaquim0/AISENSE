# 15 — Notas da Equipe

> **O problema que isto resolve:** o que um agente aprende hoje morre quando o terminal reinicia.
> Decisões, contratos de API e convenções ficam presos na janela de contexto de quem estava lá.
> As notas são a memória da equipe — arquivos Markdown que qualquer agente lê e escreve, e que
> ficam versionados junto com o projeto.

Aprovado para o v1 (decisão D6 em [ESTADO.md](ESTADO.md)).

## Onde ficam

```
<workdir>/.aisense/notes/
├── contratos-api.md
├── decisoes.md
└── convencoes.md
```

Dentro do diretório de trabalho da equipe, **de propósito**: assim entram no git do projeto e
acompanham o código. Diferente do resto de `.aisense/` (que é descartável e regenerável), as notas
são conteúdo do usuário — o app sugere `!.aisense/notes/` no `.gitignore` que gera.

Uma nota é Markdown puro. O título é o primeiro `#` do arquivo; o `slug` é o nome do arquivo.

## API dos agentes

```bash
aisense notes list                        # slug, título e quando mudou
aisense notes read contratos-api          # conteúdo completo
aisense notes read contratos-api --section "Autenticação"
aisense notes append contratos-api "POST /auth/token devolve refresh_token desde a v2"
aisense notes write contratos-api --file novo.md --expect-hash a1b2c3
aisense notes search "legacy_id"          # busca literal, com nome do arquivo e linha
aisense notes new decisoes --title "Decisões técnicas"
```

### Escrita concorrente é o ponto delicado

Dois agentes escrevendo a mesma nota ao mesmo tempo é o caso normal, não a exceção.
Duas operações, com garantias diferentes:

| Operação | Garantia |
|---|---|
| `append` | **Atômica.** Abre com `O_APPEND` e escreve de uma vez. Nunca perde conteúdo de ninguém. É o caminho recomendado e o que a skill ensina |
| `write` | Substitui o arquivo. **Exige `--expect-hash`** com o hash lido; se o arquivo mudou desde a leitura, falha com `stale_note` e mostra o diff |

`write` sem `--expect-hash` só é aceito quando a nota não existe. Sem essa trava, o segundo agente
apaga silenciosamente o trabalho do primeiro — e ninguém descobre até ser tarde.

## Integração com o boot

O `BOOT.md` recebe um **índice** das notas, não o conteúdo:

```markdown
## Memória da equipe
Estas notas são a fonte da verdade compartilhada. Leia a que for relevante antes de decidir algo.

| Nota | Assunto | Atualizada |
|---|---|---|
| contratos-api | Formato dos endpoints e mudanças entre versões | há 2h por @arquiteto |
| decisoes | Escolhas técnicas e o porquê | ontem por @voce |

Leia com: `aisense notes read <slug>`
```

Índice e não conteúdo porque o `BOOT.md` já tem limite de 12.000 caracteres
([06 — Sistema de Skills](06-sistema-de-skills.md)); despejar as notas inteiras estouraria o
contexto logo no boot e desperdiçaria o que o agente nem vai usar.

## Interface

Botão **Notas** na Sala da Equipe (o inspetor é do agente; as notas são da equipe): lista,
busca, editor Markdown com preview (o mesmo do editor de skills) e um aviso quando um agente
altera a nota que você está editando — o painel confere o disco a cada 3 s. Salvar com a nota
mudada mostra o diff e deixa escolher entre a versão do disco e a sua. Ainda por vir: histórico
de quem mudou o quê (precisa do autor, que chega pelo barramento na Fase 05) e busca pelo `⌘⇧F`
junto com terminais e mensagens (Fase 08).

## Limites

| Limite | Valor | Por quê |
|---|---|---|
| Tamanho de uma nota | 256 KB | Acima disso é documento, não nota — vira arquivo do projeto |
| Notas por equipe | 200 | Além disso o índice no `BOOT.md` deixa de ser útil |
| Linhas do índice no boot | 20 mais recentes | Cabe no orçamento de contexto |

## O que as notas NÃO são

- **Não são banco de dados.** Estado de trabalho vai no [quadro](13-quadro-kanban.md), não em nota.
- **Não são log.** Histórico de conversa é a [linha do tempo](07-barramento-comunicacao.md).
- **Não executam nada.** São texto. Uma nota não vira instrução automática: o agente decide lê-la.
