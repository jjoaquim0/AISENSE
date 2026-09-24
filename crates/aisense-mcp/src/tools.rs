//! As ferramentas MCP e o frame que cada uma manda (`docs/07`, "Servidor MCP"). Cada
//! ferramenta vira **o mesmo** `Request` que o comando equivalente da CLI — o teste de
//! contrato em `tests/contract.rs` garante.

use aisense_core::agent::Autonomy;
use aisense_core::board::{CardFilter, CardPatch, CardPriority, LinkKind, NewCard};
use aisense_core::proposal::ProposalAction;
use aisense_ipc::{NotesOp, Request, TaskOp};
use serde_json::{json, Value};

/// As ferramentas anunciadas em `tools/list`.
pub fn list() -> Value {
    json!([
        {
            "name": "aisense_list_agents",
            "description": "Lista os agentes da sua equipe: endereço, papel, runtime, estado e mensagens não lidas.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "aisense_send_message",
            "description": "Manda uma mensagem sem esperar resposta. Destinos: @agente, #canal, @all (equipe inteira) ou @voce (o humano).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": { "type": "array", "items": { "type": "string" }, "description": "Ex.: [\"@frontend\"]" },
                    "body": { "type": "string", "description": "Texto; seja específico (arquivo, função, erro)." }
                },
                "required": ["to", "body"]
            }
        },
        {
            "name": "aisense_ask_agent",
            "description": "Pergunta a um agente e ESPERA a resposta (até o timeout). Use quando a informação está com o colega.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": { "type": "string", "description": "Ex.: \"@arquiteto\"" },
                    "body": { "type": "string" },
                    "timeout_s": { "type": "integer", "minimum": 1, "maximum": 1800 }
                },
                "required": ["to", "body"]
            }
        },
        {
            "name": "aisense_reply",
            "description": "Responde uma pergunta que te fizeram (use o id da mensagem).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "reply_to": { "type": "string", "description": "Id da pergunta (msg_...)" },
                    "body": { "type": "string" }
                },
                "required": ["reply_to", "body"]
            }
        },
        {
            "name": "aisense_read_inbox",
            "description": "Lê suas mensagens não lidas, mais antiga primeiro. drain=true marca como lidas.",
            "inputSchema": {
                "type": "object",
                "properties": { "drain": { "type": "boolean" } }
            }
        },
        {
            "name": "aisense_notes",
            "description": "Notas da equipe (memória compartilhada): list, read, append, write, search, new. Prefira append: nunca conflita.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "read", "append", "write", "search", "new"] },
                    "slug": { "type": "string" },
                    "section": { "type": "string" },
                    "text": { "type": "string" },
                    "content": { "type": "string" },
                    "expect_hash": { "type": "string" },
                    "query": { "type": "string" },
                    "title": { "type": "string" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "aisense_channels",
            "description": "Canais da equipe: list (com inscritos), join ou leave. Canal sem inscritos vai para a equipe toda.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "join", "leave"] },
                    "channel": { "type": "string", "description": "#canal (join/leave)" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "aisense_propose",
            "description": "Ações estruturais (criar agente, mudar autonomia, editar skill, mudar colunas do quadro) NÃO são executadas por agentes: viram proposta para o humano aceitar ou recusar na UI. Você recebe a decisão como mensagem.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["agent", "autonomy", "skill", "columns"] },
                    "handle": { "type": "string", "description": "@handle (agent, autonomy)" },
                    "name": { "type": "string" },
                    "role": { "type": "string" },
                    "runtime": { "type": "string", "description": "claude, codex, shell... (agent)" },
                    "autonomy": { "type": "string", "enum": ["ask", "trusted"] },
                    "skill": { "type": "string" },
                    "change": { "type": "string", "description": "O que mudar (skill, columns)" },
                    "reason": { "type": "string", "description": "Por quê — é o que o humano lê" }
                },
                "required": ["kind", "reason"]
            }
        },
        {
            "name": "aisense_board",
            "description": "O quadro da equipe em texto: colunas, cartões, responsáveis, bloqueios. É a memória do trabalho — leia antes de começar.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "column": { "type": "string", "description": "Só uma coluna (slug: backlog, todo, doing, blocked, review, done)" },
                    "full": { "type": "boolean", "description": "Todos os cartões, sem resumo por coluna" }
                }
            }
        },
        {
            "name": "aisense_next_task",
            "description": "Sugere o próximo cartão que VOCÊ deveria pegar (os seus prontos primeiro, depois os sem dono e sem dependência aberta).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "aisense_list_tasks",
            "description": "Lista cartões com filtros. Concluídos ficam de fora, a menos que all=true ou column seja dada.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "column": { "type": "string" },
                    "assignee": { "type": "string", "description": "@handle" },
                    "mine": { "type": "boolean" },
                    "unassigned": { "type": "boolean" },
                    "label": { "type": "string" },
                    "all": { "type": "boolean" }
                }
            }
        },
        {
            "name": "aisense_show_task",
            "description": "Cartão completo: corpo, checklist, dependências, links, comentários e histórico.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "string", "description": "tsk_... (o id curto do quadro serve)" } },
                "required": ["id"]
            }
        },
        {
            "name": "aisense_create_task",
            "description": "Cria um cartão. Agrupe: prefira checklist a vários cartões pequenos.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "body": { "type": "string" },
                    "assign": { "type": "string", "description": "@handle" },
                    "column": { "type": "string", "description": "Padrão: todo" },
                    "labels": { "type": "array", "items": { "type": "string" } },
                    "priority": { "type": "string", "enum": ["low", "normal", "high", "urgent"] },
                    "blocked_by": { "type": "array", "items": { "type": "string" } },
                    "checklist": { "type": "array", "items": { "type": "string" } },
                    "parent": { "type": "string" }
                },
                "required": ["title"]
            }
        },
        {
            "name": "aisense_update_task",
            "description": "Age sobre um cartão. action: claim (pegar, atômico), move (column, reason), update (title, body, assign, unassign, add_labels, remove_labels, priority, blocked_by, unblock, checklist), check (item, undo), comment (body), link (kind: pr|commit|file|url, target), block (reason obrigatório), done (note), split (titles), approve (note), reject (reason obrigatório), archive.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "action": { "type": "string", "enum": ["claim", "move", "update", "check", "comment", "link", "block", "done", "split", "approve", "reject", "archive"] },
                    "column": { "type": "string" },
                    "reason": { "type": "string" },
                    "note": { "type": "string" },
                    "body": { "type": "string" },
                    "title": { "type": "string" },
                    "item": { "type": "integer", "minimum": 1 },
                    "undo": { "type": "boolean" },
                    "kind": { "type": "string", "enum": ["pr", "commit", "file", "url"] },
                    "target": { "type": "string" },
                    "titles": { "type": "array", "items": { "type": "string" } },
                    "assign": { "type": "string" },
                    "unassign": { "type": "boolean" },
                    "add_labels": { "type": "array", "items": { "type": "string" } },
                    "remove_labels": { "type": "array", "items": { "type": "string" } },
                    "priority": { "type": "string", "enum": ["low", "normal", "high", "urgent"] },
                    "blocked_by": { "type": "array", "items": { "type": "string" } },
                    "unblock": { "type": "array", "items": { "type": "string" } },
                    "checklist": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["id", "action"]
            }
        },
        {
            "name": "aisense_watch_tasks",
            "description": "Espera (sem gastar tokens) até algo mudar nos seus cartões: atribuição, comentário, rejeição.",
            "inputSchema": {
                "type": "object",
                "properties": { "timeout_s": { "type": "integer", "minimum": 1, "maximum": 1800 } }
            }
        }
    ])
}

fn strings(args: &Value, key: &str) -> Vec<String> {
    match args.get(key) {
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(one)) => vec![one.clone()],
        _ => Vec::new(),
    }
}

fn flag(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn priority(args: &Value) -> Result<Option<CardPriority>, String> {
    opt(args, "priority")
        .map(|p| CardPriority::parse(&p).ok_or_else(|| format!("prioridade desconhecida: {p}")))
        .transpose()
}

fn task(args: &Value) -> Result<TaskOp, String> {
    let id = text(args, "id")?;
    Ok(match text(args, "action")?.as_str() {
        "claim" => TaskOp::Claim { id },
        "archive" => TaskOp::Archive { id },
        "move" => TaskOp::Move {
            id,
            column: text(args, "column")?,
            reason: opt(args, "reason"),
        },
        "update" => TaskOp::Update {
            id,
            patch: CardPatch {
                title: opt(args, "title"),
                body: opt(args, "body"),
                assignee: if flag(args, "unassign") {
                    Some(String::new())
                } else {
                    opt(args, "assign")
                },
                add_labels: strings(args, "add_labels"),
                remove_labels: strings(args, "remove_labels"),
                priority: priority(args)?,
                add_checklist: strings(args, "checklist"),
                blocked_by: strings(args, "blocked_by"),
                unblock: strings(args, "unblock"),
            },
        },
        "check" => TaskOp::Check {
            id,
            item: args
                .get("item")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("faltou o campo \"item\" (número do item, a partir de 1)")?,
            undo: flag(args, "undo"),
        },
        "comment" => TaskOp::Comment {
            id,
            body: text(args, "body")?,
        },
        "link" => TaskOp::Link {
            id,
            kind: LinkKind::parse(&text(args, "kind")?)
                .ok_or("kind deve ser pr, commit, file ou url")?,
            target: text(args, "target")?,
        },
        "block" => TaskOp::Block {
            id,
            reason: opt(args, "reason").unwrap_or_default(),
        },
        "reject" => TaskOp::Reject {
            id,
            reason: opt(args, "reason").unwrap_or_default(),
        },
        "done" => TaskOp::Done {
            id,
            note: opt(args, "note"),
        },
        "approve" => TaskOp::Approve {
            id,
            note: opt(args, "note"),
        },
        "split" => TaskOp::Split {
            id,
            titles: strings(args, "titles"),
        },
        other => return Err(format!("ação desconhecida: {other}")),
    })
}

fn text(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("faltou o campo {key:?}"))
}

fn opt(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Ferramenta + argumentos → frame do barramento.
pub fn request(name: &str, args: &Value) -> Result<Request, String> {
    Ok(match name {
        "aisense_list_agents" => Request::Agents,
        "aisense_send_message" => {
            let to = match args.get("to") {
                Some(Value::Array(list)) => list
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                Some(Value::String(one)) => vec![one.clone()],
                _ => return Err("faltou o campo \"to\"".into()),
            };
            Request::Send {
                to,
                body: text(args, "body")?,
                subject: None,
                meta: Default::default(),
            }
        }
        "aisense_ask_agent" => Request::Ask {
            to: text(args, "to")?,
            body: text(args, "body")?,
            timeout_s: args
                .get("timeout_s")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
        },
        "aisense_reply" => Request::Reply {
            reply_to: text(args, "reply_to")?,
            body: text(args, "body")?,
        },
        "aisense_read_inbox" => Request::Inbox {
            drain: args.get("drain").and_then(Value::as_bool).unwrap_or(false),
        },
        "aisense_notes" => Request::Notes(match text(args, "action")?.as_str() {
            "list" => NotesOp::List,
            "read" => NotesOp::Read {
                slug: text(args, "slug")?,
                section: opt(args, "section"),
            },
            "append" => NotesOp::Append {
                slug: text(args, "slug")?,
                text: text(args, "text")?,
            },
            "write" => NotesOp::Write {
                slug: text(args, "slug")?,
                content: text(args, "content")?,
                expect_hash: opt(args, "expect_hash"),
            },
            "search" => NotesOp::Search {
                query: text(args, "query")?,
            },
            "new" => NotesOp::New {
                slug: text(args, "slug")?,
                title: opt(args, "title").unwrap_or_default(),
            },
            other => return Err(format!("ação desconhecida: {other}")),
        }),
        "aisense_channels" => match text(args, "action")?.as_str() {
            "list" => Request::Channels,
            action @ ("join" | "leave") => Request::Subscribe {
                channel: text(args, "channel")?,
                join: action == "join",
            },
            other => return Err(format!("ação desconhecida: {other}")),
        },
        "aisense_propose" => {
            let action = match text(args, "kind")?.as_str() {
                "agent" => {
                    let handle = text(args, "handle")?;
                    ProposalAction::CreateAgent {
                        name: opt(args, "name")
                            .unwrap_or_else(|| handle.trim_start_matches('@').to_owned()),
                        handle,
                        role: opt(args, "role").unwrap_or_default(),
                        adapter_id: opt(args, "runtime").unwrap_or_default(),
                    }
                }
                "autonomy" => ProposalAction::SetAutonomy {
                    handle: text(args, "handle")?,
                    autonomy: Autonomy::parse(&text(args, "autonomy")?)
                        .ok_or("autonomy deve ser ask ou trusted")?,
                },
                "skill" => ProposalAction::EditSkill {
                    skill: text(args, "skill")?,
                    change: text(args, "change")?,
                },
                "columns" => ProposalAction::ChangeColumns {
                    change: text(args, "change")?,
                },
                other => return Err(format!("kind desconhecido: {other}")),
            };
            Request::Propose {
                action,
                reason: opt(args, "reason").unwrap_or_default(),
            }
        }
        "aisense_board" => Request::Board {
            column: opt(args, "column"),
            full: flag(args, "full"),
        },
        "aisense_next_task" => Request::Task(TaskOp::Next),
        "aisense_list_tasks" => Request::Task(TaskOp::List {
            filter: CardFilter {
                column: opt(args, "column"),
                assignee: opt(args, "assignee"),
                mine: flag(args, "mine"),
                unassigned: flag(args, "unassigned"),
                label: opt(args, "label"),
                include_done: flag(args, "all"),
            },
        }),
        "aisense_show_task" => Request::Task(TaskOp::Show {
            id: text(args, "id")?,
        }),
        "aisense_create_task" => Request::Task(TaskOp::Add {
            card: NewCard {
                title: text(args, "title")?,
                body: opt(args, "body").unwrap_or_default(),
                column: opt(args, "column"),
                assignee: opt(args, "assign"),
                labels: strings(args, "labels"),
                priority: priority(args)?,
                blocked_by: strings(args, "blocked_by"),
                checklist: strings(args, "checklist"),
                parent: opt(args, "parent"),
                reason: None,
            },
        }),
        "aisense_update_task" => Request::Task(task(args)?),
        "aisense_watch_tasks" => Request::Task(TaskOp::Watch {
            timeout_s: args
                .get("timeout_s")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
        }),
        other => return Err(format!("ferramenta desconhecida: {other}")),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use aisense_ipc::cli::parse;

    fn cli(line: &[&str]) -> Request {
        let argv: Vec<String> = line.iter().map(|s| (*s).to_owned()).collect();
        parse(&argv).unwrap().command.request().unwrap()
    }

    /// Contrato da F05-09: cada ferramenta manda exatamente o frame do comando da CLI.
    #[test]
    fn ferramenta_e_comando_da_cli_mandam_o_mesmo_frame() {
        let pairs: Vec<(Request, Request)> = vec![
            (cli(&["agents"]), request("aisense_list_agents", &json!({})).unwrap()),
            (
                cli(&["send", "@frontend", "#geral", "contrato subiu"]),
                request(
                    "aisense_send_message",
                    &json!({"to": ["@frontend", "#geral"], "body": "contrato subiu"}),
                )
                .unwrap(),
            ),
            (
                cli(&["ask", "@revisor", "revisa?", "--timeout", "60"]),
                request(
                    "aisense_ask_agent",
                    &json!({"to": "@revisor", "body": "revisa?", "timeout_s": 60}),
                )
                .unwrap(),
            ),
            (
                cli(&["reply", "msg_1", "sim"]),
                request("aisense_reply", &json!({"reply_to": "msg_1", "body": "sim"})).unwrap(),
            ),
            (
                cli(&["inbox", "--drain"]),
                request("aisense_read_inbox", &json!({"drain": true})).unwrap(),
            ),
            (
                cli(&["notes", "read", "api", "--section", "Auth"]),
                request(
                    "aisense_notes",
                    &json!({"action": "read", "slug": "api", "section": "Auth"}),
                )
                .unwrap(),
            ),
            (
                cli(&["notes", "append", "api", "linha nova"]),
                request(
                    "aisense_notes",
                    &json!({"action": "append", "slug": "api", "text": "linha nova"}),
                )
                .unwrap(),
            ),
            (
                cli(&["notes", "write", "api", "--expect-hash", "abc", "novo"]),
                request(
                    "aisense_notes",
                    &json!({"action": "write", "slug": "api", "content": "novo", "expect_hash": "abc"}),
                )
                .unwrap(),
            ),
            (
                cli(&["notes", "new", "decisoes", "--title", "Decisões"]),
                request(
                    "aisense_notes",
                    &json!({"action": "new", "slug": "decisoes", "title": "Decisões"}),
                )
                .unwrap(),
            ),
        ];
        for (from_cli, from_mcp) in pairs {
            assert_eq!(from_cli, from_mcp);
        }
        // Quadro (F06-05): cada ferramenta manda o mesmo frame do comando `aisense task`.
        let board: Vec<(Vec<&str>, &str, Value)> = vec![
            (
                vec!["board", "--column", "doing"],
                "aisense_board",
                json!({"column": "doing"}),
            ),
            (vec!["task", "next"], "aisense_next_task", json!({})),
            (
                vec!["channels"],
                "aisense_channels",
                json!({"action": "list"}),
            ),
            (
                vec!["join", "#pesquisa"],
                "aisense_channels",
                json!({"action": "join", "channel": "#pesquisa"}),
            ),
            (
                vec!["task", "list", "--mine", "--label", "backend"],
                "aisense_list_tasks",
                json!({"mine": true, "label": "backend"}),
            ),
            (
                vec!["task", "show", "tsk_7K2"],
                "aisense_show_task",
                json!({"id": "tsk_7K2"}),
            ),
            (
                vec![
                    "task",
                    "add",
                    "Migrar /users",
                    "--assign",
                    "@backend",
                    "--label",
                    "backend",
                    "--priority",
                    "high",
                    "--blocked-by",
                    "tsk_7K1",
                    "--checklist",
                    "a,b",
                ],
                "aisense_create_task",
                json!({"title": "Migrar /users", "assign": "@backend", "labels": ["backend"],
                       "priority": "high", "blocked_by": ["tsk_7K1"], "checklist": ["a,b"]}),
            ),
            (
                vec!["task", "claim", "tsk_7K2"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "claim"}),
            ),
            (
                vec!["task", "move", "tsk_7K2", "blocked", "--reason", "aguarda"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "move", "column": "blocked", "reason": "aguarda"}),
            ),
            (
                vec![
                    "task",
                    "update",
                    "tsk_7K2",
                    "--assign",
                    "@frontend",
                    "--add-label",
                    "urgente",
                ],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "update", "assign": "@frontend", "add_labels": ["urgente"]}),
            ),
            (
                vec!["task", "check", "tsk_7K2", "1"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "check", "item": 1}),
            ),
            (
                vec!["task", "comment", "tsk_7K2", "o contrato mudou"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "comment", "body": "o contrato mudou"}),
            ),
            (
                vec!["task", "link", "tsk_7K2", "--commit", "a1b2c3d"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "link", "kind": "commit", "target": "a1b2c3d"}),
            ),
            (
                vec!["task", "block", "tsk_7K2", "--reason", "falta decisão"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "block", "reason": "falta decisão"}),
            ),
            (
                vec!["task", "done", "tsk_7K2", "--note", "feito"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "done", "note": "feito"}),
            ),
            (
                vec!["task", "split", "tsk_7K2", "parte 1", "parte 2"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "split", "titles": ["parte 1", "parte 2"]}),
            ),
            (
                vec!["task", "approve", "tsk_7K2", "--note", "ok"],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "approve", "note": "ok"}),
            ),
            (
                vec![
                    "task",
                    "reject",
                    "tsk_7K2",
                    "--reason",
                    "não invalida o token",
                ],
                "aisense_update_task",
                json!({"id": "tsk_7K2", "action": "reject", "reason": "não invalida o token"}),
            ),
            (
                vec!["task", "watch", "--timeout", "60"],
                "aisense_watch_tasks",
                json!({"timeout_s": 60}),
            ),
        ];
        for (line, tool, args) in board {
            assert_eq!(cli(&line), request(tool, &args).unwrap(), "{tool} {args}");
        }
        // Toda ferramenta anunciada tem mapeamento.
        for tool in list().as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            let err = request(name, &json!({})).err().unwrap_or_default();
            assert!(!err.contains("desconhecida"), "{name}");
        }
        assert!(request("aisense_send_message", &json!({"body": "x"})).is_err());
    }
}
