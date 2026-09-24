//! Frames do barramento (`docs/07`, "Transporte"; ADR 0004): um objeto JSON por linha.
//!
//! O cliente manda `{"op": "...", ...}`; o servidor responde `{"ok": true, "data": ...}` ou
//! `{"ok": false, "error": "<código>", "message": "...", "hint": "..."}`.

use aisense_core::bus::MessageMeta;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Versão do protocolo, conferida no `hello`.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// Obrigatório e primeiro em toda conexão.
    Hello {
        token: String,
        #[serde(default = "default_version")]
        v: u32,
    },
    Whoami,
    Send {
        to: Vec<String>,
        body: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<String>,
        #[serde(default)]
        meta: MessageMeta,
    },
    Ask {
        to: String,
        body: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_s: Option<u32>,
    },
    Reply {
        reply_to: String,
        body: String,
    },
    Inbox {
        #[serde(default)]
        drain: bool,
    },
    Wait {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_s: Option<u32>,
    },
    Agents,
    Status {
        state: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    Note {
        body: String,
    },
    /// Notas da equipe (F05-12): `list`, `read`, `append`, `write`, `search`, `new`.
    Notes(NotesOp),
}

fn default_version() -> u32 {
    PROTOCOL_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotesOp {
    List,
    Read {
        slug: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<String>,
    },
    Append {
        slug: String,
        text: String,
    },
    Write {
        slug: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expect_hash: Option<String>,
    },
    Search {
        query: String,
    },
    New {
        slug: String,
        #[serde(default)]
        title: String,
    },
}

impl Request {
    /// Nome da operação, para logs (nunca o corpo: pode ter qualquer coisa).
    pub fn op(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::Whoami => "whoami",
            Self::Send { .. } => "send",
            Self::Ask { .. } => "ask",
            Self::Reply { .. } => "reply",
            Self::Inbox { .. } => "inbox",
            Self::Wait { .. } => "wait",
            Self::Agents => "agents",
            Self::Status { .. } => "status",
            Self::Note { .. } => "note",
            Self::Notes(_) => "notes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Response {
    pub fn ok(data: impl Serialize) -> Self {
        match serde_json::to_value(data) {
            Ok(value) => Self {
                ok: true,
                data: Some(value),
                error: None,
                message: None,
                hint: None,
            },
            Err(e) => Self::error("internal", e.to_string(), None),
        }
    }

    pub fn error(code: &str, message: impl Into<String>, hint: Option<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(code.to_owned()),
            message: Some(message.into()),
            hint,
        }
    }
}

/// Códigos de erro do protocolo que o transporte gera por conta própria.
pub mod codes {
    pub const UNAUTHORIZED: &str = "unauthorized";
    pub const INVALID_REQUEST: &str = "invalid_request";
    pub const FRAME_TOO_LARGE: &str = "frame_too_large";
    pub const INTERNAL: &str = "internal";
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn frames_no_formato_do_doc() {
        let send: Request =
            serde_json::from_str(r#"{"op":"send","to":["@frontend"],"body":"oi"}"#).unwrap();
        assert_eq!(
            send,
            Request::Send {
                to: vec!["@frontend".into()],
                body: "oi".into(),
                subject: None,
                meta: MessageMeta::default()
            }
        );
        let notes: Request =
            serde_json::from_str(r#"{"op":"notes","action":"read","slug":"api","section":"Auth"}"#)
                .unwrap();
        assert_eq!(notes.op(), "notes");
        assert!(
            serde_json::from_str::<Request>(r#"{"op":"send","to":[],"body":"x","extra":1}"#)
                .is_err()
        );
        let ok = serde_json::to_string(&Response::ok(serde_json::json!({"id": "m"}))).unwrap();
        assert_eq!(ok, r#"{"ok":true,"data":{"id":"m"}}"#);
        let err = serde_json::to_string(&Response::error("timeout", "sem resposta", None)).unwrap();
        assert_eq!(
            err,
            r#"{"ok":false,"error":"timeout","message":"sem resposta"}"#
        );
    }
}
