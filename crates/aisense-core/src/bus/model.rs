//! Mensagem, endereço e entrega (`docs/07`, "Modelo de mensagem"; `docs/04`, tabelas de
//! comunicação).

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agent::Handle;
use crate::ids::{AgentId, ChannelId, MessageId, TeamId};
use crate::time::Millis;

/// Teto do corpo de uma mensagem. Texto para um LLM ler, não arquivo: acima disso,
/// anexe o caminho.
pub const MESSAGE_BODY_MAX: usize = 64 * 1024;
pub const CHANNEL_SLUG_MAX: usize = 32;

/// Para onde a mensagem vai, como o remetente escreveu (`docs/07`, "Endereçamento").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Address {
    /// `@backend`
    Agent(Handle),
    /// `#geral` (sem o `#`)
    Channel(String),
    /// `@all`: a equipe inteira.
    All,
    /// `@voce`: o humano, na UI.
    Human,
}

impl Address {
    /// `@handle`, `#canal`, `@all` ou `@voce`. Espaços em volta são ignorados.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        if let Some(slug) = raw.strip_prefix('#') {
            return valid_channel(slug)
                .then(|| Self::Channel(slug.to_owned()))
                .ok_or_else(|| {
                    format!(
                        "invalid channel {raw:?}: use # followed by lowercase letters, digits and hyphens"
                    )
                });
        }
        let Some(name) = raw.strip_prefix('@') else {
            return Err(format!(
                "invalid address {raw:?}: use @agent, #channel, @all or @voce"
            ));
        };
        match name {
            "all" => Ok(Self::All),
            "voce" => Ok(Self::Human),
            _ => Handle::parse(name)
                .map(Self::Agent)
                .map_err(|e| format!("invalid address {raw:?}: {e}")),
        }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent(h) => write!(f, "@{}", h.as_str()),
            Self::Channel(slug) => write!(f, "#{slug}"),
            Self::All => f.write_str("@all"),
            Self::Human => f.write_str("@voce"),
        }
    }
}

pub fn valid_channel(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= CHANNEL_SLUG_MAX
        && slug.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum MessageKind {
    #[default]
    Message,
    /// Pergunta de um `ask`: espera `response`.
    Request,
    Response,
    Event,
    /// Do próprio AISENSE (guardas, avisos).
    System,
}

impl MessageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Request => "request",
            Self::Response => "response",
            Self::Event => "event",
            Self::System => "system",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "message" => Self::Message,
            "request" => Self::Request,
            "response" => Self::Response,
            "event" => Self::Event,
            "system" => Self::System,
            _ => return None,
        })
    }
}

/// Quem mandou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum Sender {
    #[serde(rename_all = "camelCase")]
    Agent { agent_id: AgentId },
    /// `@voce`, pela UI.
    Human,
    /// O AISENSE (guardas anti-laço, avisos).
    System,
}

/// O destino guardado: exatamente um (invariante I2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum Target {
    #[serde(rename_all = "camelCase")]
    Agent { agent_id: AgentId },
    #[serde(rename_all = "camelCase")]
    Channel { channel_id: ChannelId },
    /// `@all`
    Team,
    /// `@voce`
    Human,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

/// O que não é texto vai aqui (`docs/07`): `body` fica livre para o LLM ler.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct MessageMeta {
    pub priority: Priority,
    /// Só em `request`.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_s: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Message {
    pub id: MessageId,
    pub team_id: TeamId,
    pub kind: MessageKind,
    pub from: Sender,
    pub to: Target,
    pub reply_to: Option<MessageId>,
    pub subject: Option<String>,
    pub body: String,
    pub meta: MessageMeta,
    #[ts(type = "number")]
    pub created_at: Millis,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum DeliveryState {
    /// Na caixa de entrada, ainda não lida.
    #[default]
    Pending,
    /// Chegou ao agente (injetada no terminal ou entregue pelo hook), sem confirmação de leitura.
    Delivered,
    Read,
    Failed,
    Expired,
}

impl DeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "pending" => Self::Pending,
            "delivered" => Self::Delivered,
            "read" => Self::Read,
            "failed" => Self::Failed,
            "expired" => Self::Expired,
            _ => return None,
        })
    }

    /// Ainda não lida: conta no badge e sai no `inbox`.
    pub fn is_unread(self) -> bool {
        matches!(self, Self::Pending | Self::Delivered)
    }
}

/// Uma linha de `deliveries`: o estado da mensagem para **um** destinatário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Delivery {
    pub message_id: MessageId,
    pub agent_id: AgentId,
    pub state: DeliveryState,
    #[ts(type = "number | null")]
    pub delivered_at: Option<Millis>,
    #[ts(type = "number | null")]
    pub read_at: Option<Millis>,
    pub attempts: u32,
    pub error: Option<String>,
}

impl Delivery {
    pub fn pending(message_id: &MessageId, agent_id: &AgentId) -> Self {
        Self {
            message_id: message_id.clone(),
            agent_id: agent_id.clone(),
            state: DeliveryState::Pending,
            delivered_at: None,
            read_at: None,
            attempts: 0,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Channel {
    pub id: ChannelId,
    pub team_id: TeamId,
    pub slug: String,
    pub topic: String,
    #[ts(type = "number")]
    pub created_at: Millis,
}

/// Uma mensagem da caixa de entrada: a mensagem e o estado dela para este agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct InboxItem {
    pub message: Message,
    pub delivery: Delivery,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn enderecos() {
        assert_eq!(
            Address::parse(" @backend ").unwrap(),
            Address::Agent(Handle::parse("backend").unwrap())
        );
        assert_eq!(
            Address::parse("#geral").unwrap(),
            Address::Channel("geral".into())
        );
        assert_eq!(Address::parse("@all").unwrap(), Address::All);
        assert_eq!(Address::parse("@voce").unwrap(), Address::Human);
        for bad in ["backend", "@", "#", "#Geral", "@Back", "@a b", "#a/b"] {
            assert!(Address::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(Address::parse("#deploys").unwrap().to_string(), "#deploys");
    }
}
