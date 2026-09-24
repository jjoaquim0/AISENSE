//! As operações do barramento como as fachadas as veem (`docs/07`, "Operações"). CLI e MCP
//! chegam aqui pelo socket (`aisense-ipc`); a UI, por comando Tauri. Nenhuma delas tem
//! lógica própria: validar, rotear, marcar lida e avisar quem espera acontece só aqui.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use ts_rs::TS;

use super::model::{Address, DeliveryState, InboxItem, Message, MessageKind, MessageMeta, Sender};
use super::repo::{BusRepository, InboxQuery};
use super::{route, BusError, BusResult, Outgoing, Routed};
use crate::agent::{AgentState, Handle};
use crate::ids::{AgentId, SessionId, TeamId};
use crate::repo::{AgentRepository, TeamRepository, TokenRepository};
use crate::time::{now_ms, Millis};

/// Tudo que o serviço precisa do banco.
pub trait BusStore: BusRepository + AgentRepository + TeamRepository + TokenRepository {}
impl<T: BusRepository + AgentRepository + TeamRepository + TokenRepository> BusStore for T {}

/// Quanto a caixa de entrada devolve por vez.
pub const INBOX_LIMIT: u32 = 50;
/// Teto de `wait` (`docs/07`: `ask` vai a 1800 s; `wait` segue o mesmo teto).
pub const WAIT_MAX: Duration = Duration::from_secs(1800);

/// Quem está do outro lado do socket, depois do `hello`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Identity {
    pub agent_id: AgentId,
    pub team_id: TeamId,
    pub handle: Handle,
    pub team_name: String,
    #[ts(skip)]
    #[serde(skip)]
    pub session_id: Option<SessionId>,
}

/// Uma linha de `aisense agents`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentInfo {
    pub handle: Handle,
    pub name: String,
    pub role: String,
    pub adapter_id: String,
    pub state: AgentState,
    pub unread: u32,
}

/// Quem mais precisa saber de uma mensagem (o app emite `bus:message` para a UI).
pub trait BusObserver: Send + Sync + 'static {
    fn routed(&self, _routed: &Routed) {}
    /// Entregas que mudaram de estado (lidas, injetadas).
    fn deliveries_changed(&self, _message_ids: &[crate::ids::MessageId], _agent_id: &AgentId) {}
}

/// Observador que não faz nada (testes, CLI).
pub struct NoObserver;
impl BusObserver for NoObserver {}

pub type StateFn = Arc<dyn Fn(&AgentId) -> AgentState + Send + Sync>;

pub struct BusService<S> {
    store: Arc<S>,
    state: StateFn,
    observer: Arc<dyn BusObserver>,
    /// Toda mensagem roteada: acorda `wait` e `ask` sem polling.
    hub: broadcast::Sender<Arc<Routed>>,
}

impl<S> Clone for BusService<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            state: Arc::clone(&self.state),
            observer: Arc::clone(&self.observer),
            hub: self.hub.clone(),
        }
    }
}

impl<S: BusStore> BusService<S> {
    pub fn new(store: Arc<S>, state: StateFn, observer: Arc<dyn BusObserver>) -> Self {
        let (hub, _) = broadcast::channel(1024);
        Self {
            store,
            state,
            observer,
            hub,
        }
    }

    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    fn running(&self, id: &AgentId) -> bool {
        (self.state)(id).is_running()
    }

    /// Escuta tudo que for roteado daqui em diante.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Routed>> {
        self.hub.subscribe()
    }

    /// `hello`: o token vale enquanto a sessão do agente estiver viva (I4).
    pub async fn authenticate(&self, token: &str) -> BusResult<Identity> {
        let record = self
            .store
            .find_token(token, now_ms())
            .await?
            .ok_or(BusError::Unauthorized)?;
        let agent = self
            .store
            .get_agent(&record.agent_id)
            .await?
            .ok_or(BusError::Unauthorized)?;
        let team = self
            .store
            .get_team(&agent.team_id)
            .await?
            .ok_or(BusError::Unauthorized)?;
        Ok(Identity {
            agent_id: agent.id,
            team_id: team.id,
            handle: agent.handle,
            team_name: team.name,
            session_id: Some(record.session_id),
        })
    }

    /// Roteia, avisa a UI e acorda quem espera.
    pub async fn dispatch(&self, team_id: &TeamId, out: Outgoing) -> BusResult<Routed> {
        let routed = route(&*self.store, team_id, out, now_ms(), |id| self.running(id)).await?;
        self.observer.routed(&routed);
        // Sem ninguém esperando não é erro.
        let _ = self.hub.send(Arc::new(routed.clone()));
        Ok(routed)
    }

    /// `send`: uma mensagem por endereço. Endereço inválido recusa o lote inteiro antes de
    /// gravar qualquer coisa.
    pub async fn send(
        &self,
        team_id: &TeamId,
        from: Sender,
        to: &[String],
        body: &str,
        subject: Option<String>,
        meta: MessageMeta,
    ) -> BusResult<Vec<Routed>> {
        if to.is_empty() {
            return Err(BusError::InvalidRequest(
                "say who gets the message: @agent, #channel, @all or @voce".into(),
            ));
        }
        let addresses = to
            .iter()
            .map(|raw| Address::parse(raw).map_err(BusError::InvalidRequest))
            .collect::<BusResult<Vec<_>>>()?;
        let mut sent = Vec::with_capacity(addresses.len());
        for address in addresses {
            let out = Outgoing {
                subject: subject.clone(),
                meta: meta.clone(),
                ..Outgoing::message(from.clone(), address, body)
            };
            sent.push(self.dispatch(team_id, out).await?);
        }
        Ok(sent)
    }

    /// Mensagens não lidas, mais antiga primeiro. `drain` marca todas como lidas.
    pub async fn inbox(&self, agent_id: &AgentId, drain: bool) -> BusResult<Vec<InboxItem>> {
        let query = InboxQuery {
            unread_only: true,
            after: None,
            limit: INBOX_LIMIT,
        };
        let mut items = self.store.inbox(agent_id, &query).await?;
        if drain && !items.is_empty() {
            self.mark_read(agent_id, &mut items).await?;
        }
        Ok(items)
    }

    async fn mark_read(&self, agent_id: &AgentId, items: &mut [InboxItem]) -> BusResult<()> {
        let now = now_ms();
        let ids: Vec<_> = items.iter().map(|i| i.message.id.clone()).collect();
        self.store.mark_read(agent_id, &ids, now).await?;
        for item in items.iter_mut() {
            item.delivery.state = DeliveryState::Read;
            item.delivery.read_at = Some(now);
        }
        self.observer.deliveries_changed(&ids, agent_id);
        Ok(())
    }

    /// Bloqueia até chegar mensagem para o agente (ou já haver uma não lida) e a devolve
    /// lida. `None` no timeout.
    pub async fn wait(
        &self,
        agent_id: &AgentId,
        timeout: Duration,
    ) -> BusResult<Option<InboxItem>> {
        let timeout = timeout.min(WAIT_MAX);
        // Inscreve antes de olhar a caixa: nada que chegue no meio se perde.
        let mut rx = self.subscribe();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let query = InboxQuery {
                unread_only: true,
                after: None,
                limit: 1,
            };
            let mut items = self.store.inbox(agent_id, &query).await?;
            if !items.is_empty() {
                self.mark_read(agent_id, &mut items).await?;
                return Ok(items.pop());
            }
            loop {
                match tokio::time::timeout_at(deadline, rx.recv()).await {
                    Err(_) => return Ok(None),
                    Ok(Ok(routed)) if routed.deliveries.iter().any(|d| &d.agent_id == agent_id) => {
                        break
                    }
                    Ok(Ok(_)) => {}
                    // Atrasou demais e perdeu eventos: relê a caixa.
                    Ok(Err(broadcast::error::RecvError::Lagged(_))) => break,
                    Ok(Err(broadcast::error::RecvError::Closed)) => return Ok(None),
                }
            }
        }
    }

    /// `agents`: a equipe na ordem dela, com estado e não lidas.
    pub async fn agents(&self, team_id: &TeamId) -> BusResult<Vec<AgentInfo>> {
        let unread = self.store.unread_counts(team_id).await?;
        let mut agents = self.store.list_agents(team_id).await?;
        agents.sort_by_key(|a| a.position);
        Ok(agents
            .into_iter()
            .map(|a| AgentInfo {
                state: (self.state)(&a.id),
                unread: unread
                    .iter()
                    .find(|(id, _)| id == &a.id)
                    .map_or(0, |(_, n)| *n),
                handle: a.handle,
                name: a.name,
                role: a.role,
                adapter_id: a.adapter_id,
            })
            .collect())
    }

    /// `note`: registro na linha do tempo, sem destinatário de agente.
    pub async fn note(&self, identity: &Identity, body: &str) -> BusResult<Routed> {
        let out = Outgoing {
            kind: MessageKind::Event,
            ..Outgoing::message(
                Sender::Agent {
                    agent_id: identity.agent_id.clone(),
                },
                Address::Human,
                body,
            )
        };
        self.dispatch(&identity.team_id, out).await
    }

    /// `status`: o agente diz o que está fazendo; vai para a linha do tempo.
    pub async fn status(
        &self,
        identity: &Identity,
        state: &str,
        note: Option<&str>,
    ) -> BusResult<Routed> {
        let state = state.trim();
        if state.is_empty() {
            return Err(BusError::InvalidRequest(
                "say the status, e.g. \"revisando o PR\"".into(),
            ));
        }
        let body = match note.map(str::trim).filter(|n| !n.is_empty()) {
            Some(note) => format!("status: {state} — {note}"),
            None => format!("status: {state}"),
        };
        let out = Outgoing {
            kind: MessageKind::Event,
            subject: Some("status".into()),
            ..Outgoing::message(
                Sender::Agent {
                    agent_id: identity.agent_id.clone(),
                },
                Address::Human,
                body,
            )
        };
        self.dispatch(&identity.team_id, out).await
    }

    /// A linha do tempo da equipe (UI).
    pub async fn timeline(
        &self,
        team_id: &TeamId,
        before: Option<&crate::ids::MessageId>,
        limit: u32,
    ) -> BusResult<Vec<Message>> {
        Ok(self.store.timeline(team_id, before, limit.min(500)).await?)
    }

    /// Relógio do serviço — um lugar só, para os testes poderem raciocinar sobre ele.
    pub fn now(&self) -> Millis {
        now_ms()
    }
}

/// Nomes para exibir: `@handle`, `#canal`, `@all`, `@voce`. Montado por equipe, no momento
/// da leitura — um agente renomeado aparece com o nome novo em todo o histórico.
#[derive(Debug, Clone, Default)]
pub struct Directory {
    agents: std::collections::HashMap<AgentId, String>,
    channels: std::collections::HashMap<crate::ids::ChannelId, String>,
}

/// Como o remetente aparece quando é o próprio AISENSE.
pub const SYSTEM_LABEL: &str = "aisense";
/// Agente que já foi excluído (a conversa sobrevive a ele).
pub const REMOVED_AGENT_LABEL: &str = "@removido";

impl Directory {
    pub fn sender(&self, sender: &Sender) -> String {
        match sender {
            Sender::Agent { agent_id } => self
                .agents
                .get(agent_id)
                .map_or_else(|| REMOVED_AGENT_LABEL.to_owned(), |h| format!("@{h}")),
            Sender::Human => "@voce".into(),
            Sender::System => SYSTEM_LABEL.into(),
        }
    }

    pub fn target(&self, target: &super::model::Target) -> String {
        use super::model::Target;
        match target {
            Target::Agent { agent_id } => self
                .agents
                .get(agent_id)
                .map_or_else(|| REMOVED_AGENT_LABEL.to_owned(), |h| format!("@{h}")),
            Target::Channel { channel_id } => self
                .channels
                .get(channel_id)
                .map_or_else(|| "#?".to_owned(), |s| format!("#{s}")),
            Target::Team => "@all".into(),
            Target::Human => "@voce".into(),
        }
    }
}

impl<S: BusStore> BusService<S> {
    pub async fn directory(&self, team_id: &TeamId) -> BusResult<Directory> {
        let agents = self.store.list_agents(team_id).await?;
        let channels = self.store.list_channels(team_id).await?;
        Ok(Directory {
            agents: agents
                .into_iter()
                .map(|a| (a.id, a.handle.as_str().to_owned()))
                .collect(),
            channels: channels.into_iter().map(|c| (c.id, c.slug)).collect(),
        })
    }
}

/// A mensagem como gente (e LLM) lê: nomes no lugar de ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct MessageView {
    pub id: crate::ids::MessageId,
    pub kind: MessageKind,
    /// `@backend`, `@voce`, `aisense`.
    pub from: String,
    /// `@frontend`, `#geral`, `@all`, `@voce`.
    pub to: String,
    pub subject: Option<String>,
    pub body: String,
    pub reply_to: Option<crate::ids::MessageId>,
    pub meta: MessageMeta,
    #[ts(type = "number")]
    pub created_at: Millis,
}

impl Directory {
    pub fn view(&self, message: &Message) -> MessageView {
        MessageView {
            id: message.id.clone(),
            kind: message.kind,
            from: self.sender(&message.from),
            to: self.target(&message.to),
            subject: message.subject.clone(),
            body: message.body.clone(),
            reply_to: message.reply_to.clone(),
            meta: message.meta.clone(),
            created_at: message.created_at,
        }
    }
}

/// Não lidas de um agente (badge da sidebar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct UnreadCount {
    pub agent_id: AgentId,
    pub count: u32,
}

/// Payload de `bus:message`: a mensagem já com os nomes, e a equipe dela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct BusMessageEvent {
    pub team_id: TeamId,
    pub message: MessageView,
    /// Quantos agentes receberam (entregas criadas).
    pub recipients: u32,
}

impl<S: BusStore> BusService<S> {
    pub async fn unread(&self, team_id: &TeamId) -> BusResult<Vec<UnreadCount>> {
        Ok(self
            .store
            .unread_counts(team_id)
            .await?
            .into_iter()
            .map(|(agent_id, count)| UnreadCount { agent_id, count })
            .collect())
    }

    /// A linha do tempo com nomes resolvidos (UI).
    pub async fn timeline_views(
        &self,
        team_id: &TeamId,
        before: Option<&crate::ids::MessageId>,
        limit: u32,
    ) -> BusResult<Vec<MessageView>> {
        let dir = self.directory(team_id).await?;
        let messages = self.timeline(team_id, before, limit).await?;
        Ok(messages.iter().map(|m| dir.view(m)).collect())
    }

    /// O evento para a UI de uma mensagem recém-roteada.
    pub async fn event_for(&self, routed: &Routed) -> BusResult<BusMessageEvent> {
        let dir = self.directory(&routed.message.team_id).await?;
        Ok(BusMessageEvent {
            team_id: routed.message.team_id.clone(),
            message: dir.view(&routed.message),
            recipients: u32::try_from(routed.deliveries.len()).unwrap_or(u32::MAX),
        })
    }
}
