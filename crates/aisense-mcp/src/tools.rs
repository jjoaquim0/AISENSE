//! As ferramentas MCP e o frame que cada uma manda (`docs/07`, "Servidor MCP"). Cada
//! ferramenta vira **o mesmo** `Request` que o comando equivalente da CLI — o teste de
//! contrato em `tests/contract.rs` garante.

use aisense_ipc::{NotesOp, Request};
use serde_json::{json, Value};

/// As ferramentas anunciadas em `tools/list`. As de tarefas (`aisense_create_task`,
/// `aisense_update_task`) entram com o quadro, na Fase 06.
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
        }
    ])
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
        // Toda ferramenta anunciada tem mapeamento.
        for tool in list().as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            let err = request(name, &json!({})).err().unwrap_or_default();
            assert!(!err.contains("desconhecida"), "{name}");
        }
        assert!(request("aisense_send_message", &json!({"body": "x"})).is_err());
    }
}
