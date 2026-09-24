//! Saída em texto para LLM e para gente: limpa, sem cor, uma ideia por linha.

use serde_json::Value;

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Uma mensagem (`MessageView`).
fn message(m: &Value) -> String {
    let kind = match s(m, "kind") {
        "request" => " · pergunta",
        "response" => " · resposta",
        "system" => " · sistema",
        "event" => " · registro",
        _ => "",
    };
    let subject = match s(m, "subject") {
        "" => String::new(),
        subject => format!(" · {subject}"),
    };
    let mut out = format!(
        "── {} → {}{kind}{subject} · {}\n",
        s(m, "from"),
        s(m, "to"),
        s(m, "id")
    );
    out.push_str(s(m, "body"));
    out.push('\n');
    if s(m, "kind") == "request" {
        out.push_str(&format!(
            "(responda com: aisense reply {} \"...\")\n",
            s(m, "id")
        ));
    }
    out
}

fn state(raw: &str) -> &str {
    match raw {
        "stopped" => "parado",
        "starting" => "iniciando",
        "idle" => "ocioso",
        "busy" => "ocupado",
        "awaitingInput" => "aguardando o humano",
        "failed" => "caiu",
        other => other,
    }
}

pub fn render(op: &str, data: &Value, if_any: bool) -> String {
    match op {
        "send" => {
            let n = data["ids"].as_array().map_or(0, Vec::len);
            let mut out = if n == 1 {
                format!("enviado ({})\n", data["ids"][0].as_str().unwrap_or(""))
            } else {
                format!("enviado ({n} mensagens)\n")
            };
            for handle in data["stopped"].as_array().into_iter().flatten() {
                out.push_str(&format!(
                    "@{} está parado: lê quando voltar.\n",
                    handle.as_str().unwrap_or("")
                ));
            }
            out
        }
        "inbox" => {
            let items = data.as_array().cloned().unwrap_or_default();
            if items.is_empty() {
                return if if_any {
                    String::new()
                } else {
                    "Nenhuma mensagem nova.\n".into()
                };
            }
            items.iter().map(message).collect::<Vec<_>>().join("\n")
        }
        "wait" | "ask" => message(data),
        "agents" => data
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| {
                let unread = a["unread"].as_u64().unwrap_or(0);
                let unread = if unread > 0 {
                    format!(" · {unread} não lida(s)")
                } else {
                    String::new()
                };
                let role = s(a, "role").lines().next().unwrap_or("");
                format!(
                    "@{:<16} ({}, {}){unread}  {role}\n",
                    s(a, "handle"),
                    s(a, "adapterId"),
                    state(s(a, "state"))
                )
            })
            .collect(),
        "whoami" => format!(
            "@{} — equipe {} ({})\n",
            s(data, "handle"),
            s(data, "teamName"),
            s(data, "agentId")
        ),
        "status" | "note" | "reply" => format!("registrado ({})\n", s(data, "id")),
        "notes" => notes(data),
        _ => format!("{data}\n"),
    }
}

fn notes(data: &Value) -> String {
    // Lista (list), ocorrências (search), nota inteira (read/append/write/new) ou seção.
    if let Some(items) = data.as_array() {
        if items.is_empty() {
            return "Nada.\n".into();
        }
        return items
            .iter()
            .map(|i| {
                if i.get("line").is_some() {
                    format!("{}:{}: {}\n", s(i, "slug"), i["line"], s(i, "text"))
                } else {
                    format!("{:<24} {}\n", s(i, "slug"), s(i, "title"))
                }
            })
            .collect();
    }
    let mut out = s(data, "content").to_owned();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if let Some(hash) = data.get("hash").and_then(Value::as_str) {
        out.push_str(&format!(
            "(hash {hash} — use em --expect-hash para substituir)\n"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mensagem_legivel_com_dica_de_resposta() {
        let m = json!({"id": "msg_1", "kind": "request", "from": "@backend", "to": "@revisor",
                       "body": "revisa o diff?", "subject": null});
        let text = render("wait", &m, false);
        assert!(text.starts_with("── @backend → @revisor · pergunta · msg_1\nrevisa o diff?\n"));
        assert!(text.contains("aisense reply msg_1"));
        assert_eq!(render("inbox", &json!([]), true), "");
        assert_eq!(
            render("inbox", &json!([]), false),
            "Nenhuma mensagem nova.\n"
        );
        let sent = render(
            "send",
            &json!({"ids": ["msg_2"], "stopped": ["frontend"]}),
            false,
        );
        assert_eq!(
            sent,
            "enviado (msg_2)\n@frontend está parado: lê quando voltar.\n"
        );
    }
}
