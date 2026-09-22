# 17 — Comandos do Projeto

> **O problema que isto resolve:** cada agente inventa por conta própria como rodar os testes do
> projeto. Um usa `npm test`, outro `pnpm test`, outro tenta `cargo test` num projeto Node.
> Erram, tentam de novo, queimam contexto — e o resultado varia entre agentes.

Um arquivo na raiz do repositório diz, uma vez, como o projeto se opera.
Aprovado para o v1 (decisão D6 em [ESTADO.md](ESTADO.md)).

## `aisense.toml`

```toml
[project]
name = "minha-api"

[commands]
install = "pnpm install"
dev     = "pnpm dev"
test    = "pnpm test"
lint    = "pnpm lint"
build   = "pnpm build"
# comandos livres também valem
migrate = "pnpm prisma migrate dev"

[bench]
# Arquivos ignorados pelo git que cada bancada precisa para funcionar (ver doc 16).
copy = [".env", ".env.local"]
# Comando rodado uma vez ao criar a bancada.
setup = "pnpm install"

[gates]
# O quadro pode exigir que estes comandos passem antes de mover o cartão (ver doc 13).
review = ["lint", "test"]
```

## API dos agentes

```bash
aisense run test                  # executa, transmite a saída, devolve o exit code do comando
aisense run test --json           # { "command": "pnpm test", "exit_code": 0, "duration_ms": 8210 }
aisense commands                  # lista o que existe, para o agente não adivinhar
```

**`aisense run` só executa nomes definidos no arquivo.** Nunca aceita uma string de comando
arbitrária. A diferença importa: o conjunto de comandos é revisável no git como qualquer código,
enquanto um `aisense run "curl ... | sh"` seria uma porta de execução arbitrária aberta pelo
barramento ([11 — Segurança](11-seguranca.md)).

## Integração com o boot

O `BOOT.md` ganha a lista, para o agente saber sem perguntar:

```markdown
## Comandos deste projeto
| Comando | Faz |
|---|---|
| `aisense run install` | pnpm install |
| `aisense run test` | pnpm test |
| `aisense run lint` | pnpm lint |

Use estes em vez de adivinhar o gerenciador de pacotes do projeto.
```

## Detecção automática

Sem `aisense.toml`, o app inspeciona o diretório e **propõe** um (nunca cria sozinho):

| Encontrou | Propõe |
|---|---|
| `pnpm-lock.yaml` | `pnpm install/test/lint/build` |
| `package-lock.json` | `npm ci`, `npm test`, … |
| `Cargo.toml` | `cargo build/test/clippy` |
| `pyproject.toml` + `uv.lock` | `uv sync`, `uv run pytest` |
| `Makefile` | os alvos que existirem |

A proposta aparece na UI com o TOML gerado, para você revisar e aceitar.

## Execução

- Roda no diretório de trabalho do agente (a bancada dele, quando houver).
- Herda o ambiente do agente, incluindo as variáveis do AISENSE.
- Timeout padrão de 10 min, configurável por comando (`test = { run = "...", timeout_s = 1800 }`).
- Saída transmitida ao terminal **e** registrada, para o quadro poder mostrar por que um gate falhou.
- Concorrência: o mesmo comando não roda duas vezes ao mesmo tempo no mesmo diretório — a segunda
  chamada espera ou falha com `already_running`, conforme a configuração.
