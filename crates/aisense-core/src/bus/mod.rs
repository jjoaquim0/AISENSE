//! Barramento de comunicação (`docs/07`, Fase 05): quem manda o quê para quem.
//!
//! `route` resolve o endereço contra a equipe, valida e grava a mensagem com uma entrega
//! por destinatário (invariante I3). Não abre socket nem PTY: o servidor IPC, a CLI, o MCP
//! e a UI chamam esta mesma função — nenhuma fachada tem lógica própria.

mod guards;
mod hook;
mod model;
mod push;
mod repo;
mod service;

pub use guards::{BlockReason, BusBlocked, GuardConfig};
pub use hook::{
    hook_output, install_inbox_hook, install_mcp_server, HookInstall, INBOX_HOOK_COMMAND,
    MCP_SERVER_NAME,
};
pub use model::{
    valid_channel, Address, Channel, Delivery, DeliveryState, InboxItem, Message, MessageKind,
    MessageMeta, Priority, Sender, Target, CHANNEL_SLUG_MAX, MESSAGE_BODY_MAX,
};
pub use push::{
    compose, inject, push_item, sanitize, InjectStyle, Injection, PtyWriter, PushInjected,
    PushItem, PushQueues, PUSH_THROTTLE,
};
pub use repo::{BusRepository, InboxQuery};
pub use service::{
    AgentInfo, BusMessageEvent, BusObserver, BusService, BusStore, Directory, Identity,
    MessageView, NoObserver, StateFn, UnreadCount, ASK_DEFAULT, ASK_MAX, INBOX_LIMIT,
    REMOVED_AGENT_LABEL, SYSTEM_LABEL, WAIT_MAX,
};

use crate::agent::{Agent, Handle};
use crate::ids::{AgentId, ChannelId, MessageId, TeamId};
use crate::repo::{AgentRepository, RepoError};
use crate::time::Millis;

#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("invalid, expired or missing AISENSE_TOKEN")]
    Unauthorized,
    #[error("there is no agent @{0} in this team")]
    UnknownAgent(String),
    #[error("@{0} is stopped")]
    AgentStopped(String),
    #[error("message {0} not found in this team")]
    UnknownMessage(MessageId),
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{detail}")]
    Blocked {
        reason: BlockReason,
        detail: String,
        hint: String,
    },
    #[error("no answer from @{handle} in {secs} s")]
    Timeout { handle: String, secs: u64 },
    #[error("@{target} is waiting for an answer from you: asking back would lock both ({cycle})")]
    WouldDeadlock { target: String, cycle: String },
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl BusError {
    /// Os códigos do protocolo (`docs/07`, "Operações").
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::UnknownAgent(_) => "unknown_agent",
            Self::AgentStopped(_) => "agent_stopped",
            Self::UnknownMessage(_) => "unknown_message",
            Self::InvalidRequest(_) => "invalid_request",
            Self::Timeout { .. } => "timeout",
            Self::Blocked { reason, .. } => match reason {
                BlockReason::RateLimited { .. } => "rate_limited",
                BlockReason::Repeated { .. } => "repeated_message",
                BlockReason::TeamBudget { .. } => "team_paused",
            },
            Self::WouldDeadlock { .. } => "would_deadlock",
            Self::Repo(_) => "internal",
        }
    }

    /// O próximo passo, pronto para a CLI imprimir.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Unauthorized => Some(
                "Rode o comando de dentro de um terminal de agente aberto pelo AISENSE.".into(),
            ),
            Self::UnknownAgent(_) => Some("Veja quem está na equipe com: aisense agents".into()),
            Self::Blocked { hint, .. } => Some(hint.clone()),
            Self::Timeout { .. } => {
                Some("Siga sem a resposta ou tente de novo; um recado com aisense send não bloqueia.".into())
            }
            Self::WouldDeadlock { target, .. } => Some(format!(
                "Responda a pergunta de @{target} primeiro (aisense inbox) ou mande um recado com aisense send."
            )),
            Self::AgentStopped(handle) => Some(format!(
                "Deixe um recado que @{handle} lê ao voltar: aisense send @{handle} \"...\""
            )),
            _ => None,
        }
    }
}

pub type BusResult<T> = Result<T, BusError>;

/// Uma mensagem a mandar, antes de resolvida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub from: Sender,
    pub to: Address,
    pub kind: MessageKind,
    pub body: String,
    pub subject: Option<String>,
    pub reply_to: Option<MessageId>,
    pub meta: MessageMeta,
}

impl Outgoing {
    pub fn message(from: Sender, to: Address, body: impl Into<String>) -> Self {
        Self {
            from,
            to,
            kind: MessageKind::Message,
            body: body.into(),
            subject: None,
            reply_to: None,
            meta: MessageMeta::default(),
        }
    }
}

/// O que `route` gravou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed {
    pub message: Message,
    pub deliveries: Vec<Delivery>,
    /// Destinatários parados: a mensagem fica na caixa deles até voltarem.
    pub stopped: Vec<Handle>,
}

/// Resolve, valida e grava. `running` diz se um agente tem processo vivo.
pub async fn route<S>(
    store: &S,
    team_id: &TeamId,
    out: Outgoing,
    now: Millis,
    running: impl Fn(&AgentId) -> bool,
) -> BusResult<Routed>
where
    S: BusRepository + AgentRepository,
{
    let body = out.body.trim_end().to_owned();
    if body.trim().is_empty() {
        return Err(BusError::InvalidRequest("the message is empty".into()));
    }
    if body.len() > MESSAGE_BODY_MAX {
        return Err(BusError::InvalidRequest(format!(
            "the message is larger than {} KB; save it to a file and send the path",
            MESSAGE_BODY_MAX / 1024
        )));
    }
    let team = store.list_agents(team_id).await?;
    let sender_id = match &out.from {
        Sender::Agent { agent_id } => {
            if !team.iter().any(|a| &a.id == agent_id) {
                // Mensagens não atravessam equipes (`docs/07`, "Escopo").
                return Err(BusError::InvalidRequest(
                    "the sender is not part of this team".into(),
                ));
            }
            Some(agent_id)
        }
        Sender::Human | Sender::System => None,
    };

    if let Some(reply_to) = &out.reply_to {
        match store.get_message(reply_to).await? {
            Some(original) if &original.team_id == team_id => {}
            _ => return Err(BusError::UnknownMessage(reply_to.clone())),
        }
    }
    if out.kind == MessageKind::Response && out.reply_to.is_none() {
        return Err(BusError::InvalidRequest(
            "a response needs the id of the question".into(),
        ));
    }

    let others = || -> Vec<&Agent> { team.iter().filter(|a| Some(&a.id) != sender_id).collect() };
    let (target, recipients): (Target, Vec<&Agent>) = match &out.to {
        Address::Agent(handle) => {
            let agent = team
                .iter()
                .find(|a| &a.handle == handle)
                .ok_or_else(|| BusError::UnknownAgent(handle.as_str().to_owned()))?;
            if Some(&agent.id) == sender_id {
                return Err(BusError::InvalidRequest(
                    "you cannot send a message to yourself".into(),
                ));
            }
            if out.kind == MessageKind::Request && !running(&agent.id) {
                // Uma pergunta a quem está parado esperaria o timeout inteiro à toa.
                return Err(BusError::AgentStopped(handle.as_str().to_owned()));
            }
            (
                Target::Agent {
                    agent_id: agent.id.clone(),
                },
                vec![agent],
            )
        }
        Address::Channel(slug) => {
            let channel = channel_for(store, team_id, slug, now).await?;
            (
                Target::Channel {
                    channel_id: channel.id,
                },
                others(),
            )
        }
        Address::All => (Target::Team, others()),
        Address::Human => (Target::Human, Vec::new()),
    };
    if out.kind == MessageKind::Request && !matches!(target, Target::Agent { .. }) {
        return Err(BusError::InvalidRequest(
            "a question (ask) goes to one agent".into(),
        ));
    }

    let message = Message {
        id: MessageId::new(),
        team_id: team_id.clone(),
        kind: out.kind,
        from: out.from,
        to: target,
        reply_to: out.reply_to,
        subject: out.subject.filter(|s| !s.trim().is_empty()),
        body,
        meta: out.meta,
        created_at: now,
    };
    let deliveries: Vec<Delivery> = recipients
        .iter()
        .map(|a| Delivery::pending(&message.id, &a.id))
        .collect();
    store.insert_message(&message, &deliveries).await?;
    let stopped = recipients
        .iter()
        .filter(|a| !running(&a.id))
        .map(|a| a.handle.clone())
        .collect();
    Ok(Routed {
        message,
        deliveries,
        stopped,
    })
}

/// O canal da equipe, criado no primeiro uso: canais são baratos e não têm dono.
async fn channel_for<S: BusRepository>(
    store: &S,
    team_id: &TeamId,
    slug: &str,
    now: Millis,
) -> BusResult<Channel> {
    if let Some(channel) = store.channel_by_slug(team_id, slug).await? {
        return Ok(channel);
    }
    let channel = Channel {
        id: ChannelId::new(),
        team_id: team_id.clone(),
        slug: slug.to_owned(),
        topic: String::new(),
        created_at: now,
    };
    match store.create_channel(&channel).await {
        Ok(()) => Ok(channel),
        // Outro agente criou no mesmo instante: usa o dele.
        Err(RepoError::AlreadyExists(_)) => store
            .channel_by_slug(team_id, slug)
            .await?
            .ok_or_else(|| BusError::InvalidRequest(format!("channel #{slug} vanished"))),
        Err(e) => Err(e.into()),
    }
}

/// O endereço de volta de uma mensagem: o remetente dela.
pub async fn reply_address<S>(store: &S, original: &Message) -> BusResult<Address>
where
    S: AgentRepository,
{
    match &original.from {
        Sender::Agent { agent_id } => {
            let agent = store
                .get_agent(agent_id)
                .await?
                .ok_or_else(|| BusError::UnknownAgent(agent_id.to_string()))?;
            Ok(Address::Agent(agent.handle))
        }
        Sender::Human => Ok(Address::Human),
        Sender::System => Err(BusError::InvalidRequest(
            "system messages cannot be answered".into(),
        )),
    }
}

#[cfg(test)]
mod tests;
