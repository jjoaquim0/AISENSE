# ADR 0004 — Barramento em NDJSON sobre socket local

- **Status:** aceito
- **Data:** 2026-09-22
- **Fase:** 00

## Contexto

Processos de agentes (que rodam **dentro** dos PTYs) precisam falar com o core do AISENSE para
enviar e ler mensagens. Esses processos são efêmeros: a IA executa `aisense send ...`, o comando
roda em milissegundos e morre. Alguns precisam ficar bloqueados esperando (`ask`, `wait`).

Requisitos: local apenas, autenticado por agente, latência baixa, suporte a chamada bloqueante,
cliente minúsculo e rápido de iniciar, funciona nos 3 SOs.

## Opções consideradas

| Opção | Prós | Contras |
|---|---|---|
| **NDJSON sobre UDS / named pipe** | Local por construção; permissão pelo SO; cliente trivial; frame fácil de depurar com `cat`; suporta conexão longa | Precisa abstrair UDS vs named pipe (resolvido por `interprocess`) |
| HTTP local (localhost:porta) | Cliente trivial em qualquer linguagem | Porta visível a **qualquer** processo da máquina; precisa gerenciar porta, CORS e conflito; overhead por requisição |
| gRPC | Contrato forte; streaming | Peso e complexidade injustificados; cliente deixa de ser minúsculo |
| Arquivos em diretório | Simplicidade absoluta | Sem push; sem request/reply; polling; corrida de escrita |
| MCP como único caminho | Nativo para IA moderna | Exclui shell puro e runtimes sem MCP — mata o caso de uso "chame a IA que você quiser" |

## Decisão

**NDJSON sobre Unix domain socket (macOS/Linux) e named pipe (Windows)**, com autenticação por
token efêmero por sessão de agente, entregue via variável de ambiente.
MCP e CLI são **duas fachadas** sobre esse mesmo transporte.

## Consequências

**Positivas**
- Funciona em qualquer terminal, inclusive `bash` puro — o requisito central do produto.
- Sem porta de rede: nada escutando em interface alguma.
- Frames legíveis: depurar é `socat`/`nc` + olhar as linhas.
- `ask` e `wait` são conexões abertas, sem polling.

**Negativas / custos aceitos**
- Nada de acesso remoto (por design; ver D1/D4).
- Sem schema forte como no gRPC. Mitigado por structs `serde` compartilhados e testes de contrato
  entre CLI, MCP e servidor.

**O que passa a ser proibido**
- Abrir porta TCP para o barramento.
- A CLI falar com o SQLite direto — tudo passa pelo socket e pelo core.

## Quando revisitar

Se agentes remotos (D1) entrarem no escopo. Aí o caminho é um transporte adicional (TLS mútuo),
mantendo o mesmo protocolo de frames.
