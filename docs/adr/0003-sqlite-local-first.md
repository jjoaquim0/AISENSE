# ADR 0003 — SQLite local-first, sem backend em nuvem no v1

- **Status:** aceito
- **Data:** 2026-09-22
- **Fase:** 00

## Contexto

O AISENSE guarda equipes, agentes, skills, mensagens e tarefas. Mensagens crescem rápido
(uma equipe ativa gera milhares por dia) e são consultadas por intervalo de tempo e por agente.

## Opções consideradas

| Opção | Prós | Contras |
|---|---|---|
| **SQLite (SQLx)** | Zero config; transacional; índices; ótimo para log temporal; arquivo único fácil de backup | Sem sincronização entre máquinas |
| JSON/TOML em disco | Simples; legível e editável à mão | Sem transação; reescrita completa a cada mensagem; inviável para milhares de linhas |
| Backend em nuvem | Sincronização; multiusuário | Exige conta e rede; contraria o princípio local-first; custo operacional; trabalho enorme no v1 |
| Embedded KV (sled/redb) | Rápido | Consultas por intervalo e agregação viram código manual |

## Decisão

**SQLite** em `~/.aisense/aisense.db`, acessado por SQLx com verificação de query em tempo de
compilação e migrações versionadas. WAL ligado.

## Consequências

**Positivas**
- A linha do tempo é uma query indexada, não um scan.
- Backup e "exportar equipe" são operações de arquivo.
- Sem conta, sem rede, funciona offline — coerente com o princípio de produto.

**Negativas / custos aceitos**
- Sem sincronização entre máquinas no v1. Aceito: o trabalho dos agentes acontece no
  **repositório do usuário**, que já é versionado por git — é lá que o valor persiste.
- Migrações precisam ser cuidadosas: não há janela de manutenção em app de desktop.

**O que passa a ser proibido**
- Guardar buffer de terminal no banco (vai para arquivo + RAM).
- Escrever SQL fora de `aisense-store`.

## Quando revisitar

Quando houver demanda real por multiusuário ou multi-máquina. Nesse caso o caminho é um serviço de
sincronização sobre o mesmo esquema, não trocar o SQLite.
