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

/// Um canal com os inscritos (`aisense channels`, editor de canais).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct ChannelInfo {
    pub channel: super::model::Channel,
    /// Vazio = canal aberto: vai para a equipe toda.
    pub members: Vec<Handle>,
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
    /// Uma guarda anti-laço barrou um agente (F05-10). Chamado uma vez por episódio.
    fn blocked(&self, _event: &super::guards::BusBlocked) {}
}

/// Observador que não faz nada (testes, CLI).
pub struct NoObserver;
impl BusObserver for NoObserver {}

pub type StateFn = Arc<dyn Fn(&AgentId) -> AgentState + Send + Sync>;

/// `ask` sem timeout informado (`docs/07`).
pub const ASK_DEFAULT: Duration = Duration::from_secs(300);
/// Teto de `ask`.
pub const ASK_MAX: Duration = Duration::from_secs(1800);

/// Quem está bloqueado esperando quem: o grafo onde um ciclo é deadlock (F05-06).
#[derive(Default)]
struct WaitGraph {
    /// Uma aresta por `ask` em andamento: quem pergunta → quem precisa responder.
    edges: Vec<(AgentId, AgentId)>,
}

impl WaitGraph {
    /// O caminho de `from` até `to` seguindo as esperas, se existir.
    fn path(&self, from: &AgentId, to: &AgentId) -> Option<Vec<AgentId>> {
        let mut stack = vec![vec![from.clone()]];
        let mut seen = std::collections::HashSet::new();
        while let Some(path) = stack.pop() {
            let last = path.last()?.clone();
            if &last == to {
                return Some(path);
            }
            if !seen.insert(last.clone()) {
                continue;
            }
            for (_, next) in self.edges.iter().filter(|(a, _)| a == &last) {
                let mut longer = path.clone();
                longer.push(next.clone());
                stack.push(longer);
            }
        }
        None
    }

    fn remove(&mut self, edge: &(AgentId, AgentId)) {
        if let Some(at) = self.edges.iter().position(|e| e == edge) {
            self.edges.swap_remove(at);
        }
    }
}

/// Tira a aresta do grafo quando o `ask` termina, do jeito que for (resposta, timeout,
/// conexão caída).
struct Waiting {
    graph: Arc<std::sync::Mutex<WaitGraph>>,
    edge: (AgentId, AgentId),
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.graph
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.edge);
    }
}

pub struct BusService<S> {
    store: Arc<S>,
    state: StateFn,
    observer: Arc<dyn BusObserver>,
    /// Toda mensagem roteada: acorda `wait` e `ask` sem polling.
    hub: broadcast::Sender<Arc<Routed>>,
    waits: Arc<std::sync::Mutex<WaitGraph>>,
    guards: Arc<std::sync::Mutex<super::guards::GuardState>>,
}

impl<S> Clone for BusService<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            state: Arc::clone(&self.state),
            observer: Arc::clone(&self.observer),
            hub: self.hub.clone(),
            waits: Arc::clone(&self.waits),
            guards: Arc::clone(&self.guards),
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
            waits: Arc::default(),
            guards: Arc::default(),
        }
    }

    /// Com limites próprios (Configurações → Equipe; testes).
    pub fn with_guards(self, config: super::guards::GuardConfig) -> Self {
        Self {
            guards: Arc::new(std::sync::Mutex::new(
                super::guards::GuardState::with_config(config),
            )),
            ..self
        }
    }

    fn guards(&self) -> std::sync::MutexGuard<'_, super::guards::GuardState> {
        self.guards
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// O humano libera a equipe pausada pelo orçamento.
    pub fn resume_team(&self, team_id: &TeamId) {
        self.guards().resume(team_id);
    }

    pub fn team_paused(&self, team_id: &TeamId) -> bool {
        self.guards().is_paused(team_id)
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

    /// Roteia, avisa a UI e acorda quem espera. Mensagem de agente passa antes pelas
    /// guardas anti-laço (F05-10); humano e sistema não.
    pub async fn dispatch(&self, team_id: &TeamId, out: Outgoing) -> BusResult<Routed> {
        let agent_sender = match &out.from {
            Sender::Agent { agent_id } => Some(agent_id.clone()),
            _ => None,
        };
        if let Some(agent_id) = &agent_sender {
            let verdict =
                self.guards()
                    .admit(team_id, agent_id, &out.to.to_string(), &out.body, now_ms());
            if let super::guards::Verdict::Block(reason) = verdict {
                return Err(self.block(team_id, agent_id, reason).await);
            }
        }
        let depth = match (&agent_sender, &out.reply_to) {
            (Some(_), Some(parent)) => self.reply_depth(parent).await,
            _ => 0,
        };
        let routed = route(&*self.store, team_id, out, now_ms(), |id| self.running(id)).await?;
        self.observer.routed(&routed);
        // Sem ninguém esperando não é erro.
        let _ = self.hub.send(Arc::new(routed.clone()));
        let max_depth = self.guards().config().max_reply_depth;
        if let Some(agent_id) = agent_sender.filter(|_| depth == max_depth) {
            // Cadeia longa: a `max_depth`-ésima resposta passa, mas o remetente recebe um aviso
            // (uma vez, na profundidade exata, para não virar mais uma mensagem por volta).
            let warning = Outgoing {
                kind: MessageKind::System,
                ..Outgoing::message(
                    Sender::System,
                    Address::Agent(self.handle_of(&agent_id).await?),
                    format!(
                        "Esta conversa já tem {max_depth} respostas encadeadas. Resuma o que foi decidido, \
                         decida e siga — ou registre o impasse com aisense note para o humano."
                    ),
                )
            };
            let _ = Box::pin(self.dispatch(team_id, warning)).await;
        }
        Ok(routed)
    }

    async fn handle_of(&self, agent_id: &AgentId) -> BusResult<Handle> {
        self.store
            .get_agent(agent_id)
            .await?
            .map(|a| a.handle)
            .ok_or_else(|| BusError::UnknownAgent(agent_id.to_string()))
    }

    /// Quantas respostas há acima de `parent` (0 para uma mensagem que não responde nada).
    async fn reply_depth(&self, parent: &crate::ids::MessageId) -> u32 {
        let limit = self.guards().config().max_reply_depth + 1;
        let mut depth = 1;
        let mut current = parent.clone();
        while depth < limit {
            match self.store.get_message(&current).await {
                Ok(Some(message)) => match message.reply_to {
                    Some(up) => {
                        depth += 1;
                        current = up;
                    }
                    None => break,
                },
                _ => break,
            }
        }
        depth
    }

    /// Registra o bloqueio: na primeira vez do episódio, avisa a UI e deixa uma mensagem de
    /// sistema na linha do tempo — nenhum bloqueio é silencioso.
    async fn block(
        &self,
        team_id: &TeamId,
        agent_id: &AgentId,
        reason: super::guards::BlockReason,
    ) -> BusError {
        let handle = self
            .handle_of(agent_id)
            .await
            .map_or_else(|_| "?".to_owned(), |h| h.as_str().to_owned());
        let detail = reason.describe(&handle);
        let hint = reason.hint();
        if self.guards().first_block(team_id, agent_id, &reason) {
            tracing::warn!(agent = %agent_id, %detail, "guarda anti-laço");
            self.observer.blocked(&super::guards::BusBlocked {
                team_id: team_id.clone(),
                agent_id: agent_id.clone(),
                reason: reason.clone(),
                detail: detail.clone(),
            });
            let notice = Outgoing {
                kind: MessageKind::System,
                subject: Some("guarda anti-laço".into()),
                ..Outgoing::message(Sender::System, Address::Human, detail.clone())
            };
            if let Err(error) = Box::pin(self.dispatch(team_id, notice)).await {
                tracing::warn!(%error, "aviso de bloqueio não registrado");
            }
        }
        BusError::Blocked {
            reason,
            detail,
            hint,
        }
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

    // ───────────────────────────── canais (F07-05) ─────────────────────────────

    /// Os canais da equipe, com quem está inscrito. Sem inscritos = aberto a todos.
    pub async fn channels(&self, team_id: &TeamId) -> BusResult<Vec<ChannelInfo>> {
        let agents = self.store.list_agents(team_id).await?;
        let mut out = Vec::new();
        for channel in self.store.list_channels(team_id).await? {
            let members = self
                .store
                .channel_members(&channel.id)
                .await?
                .iter()
                .filter_map(|id| agents.iter().find(|a| &a.id == id))
                .map(|a| a.handle.clone())
                .collect();
            out.push(ChannelInfo { channel, members });
        }
        Ok(out)
    }

    async fn channel(&self, team_id: &TeamId, raw: &str) -> BusResult<super::model::Channel> {
        let slug = raw.trim().trim_start_matches('#');
        self.store
            .channel_by_slug(team_id, slug)
            .await?
            .ok_or_else(|| BusError::InvalidRequest(format!("there is no channel #{slug}")))
    }

    async fn member_ids(&self, team_id: &TeamId, handles: &[String]) -> BusResult<Vec<AgentId>> {
        let agents = self.store.list_agents(team_id).await?;
        handles
            .iter()
            .map(|raw| {
                let name = raw.trim().trim_start_matches('@');
                agents
                    .iter()
                    .find(|a| a.handle.as_str() == name)
                    .map(|a| a.id.clone())
                    .ok_or_else(|| BusError::UnknownAgent(name.to_owned()))
            })
            .collect()
    }

    /// Cria (ou atualiza, se já existe) um canal com tópico e inscritos.
    pub async fn save_channel(
        &self,
        team_id: &TeamId,
        slug: &str,
        topic: &str,
        members: &[String],
    ) -> BusResult<ChannelInfo> {
        let slug = slug.trim().trim_start_matches('#');
        if !super::model::valid_channel(slug) {
            return Err(BusError::InvalidRequest(format!(
                "invalid channel #{slug}: use lowercase letters, digits and hyphens"
            )));
        }
        let ids = self.member_ids(team_id, members).await?;
        let channel = match self.store.channel_by_slug(team_id, slug).await? {
            Some(channel) => channel,
            None => {
                let channel = super::model::Channel {
                    id: crate::ids::ChannelId::new(),
                    team_id: team_id.clone(),
                    slug: slug.to_owned(),
                    topic: String::new(),
                    created_at: now_ms(),
                };
                self.store.create_channel(&channel).await?;
                channel
            }
        };
        self.store
            .set_channel_topic(&channel.id, topic.trim())
            .await?;
        self.store.set_channel_members(&channel.id, &ids).await?;
        self.channels(team_id)
            .await?
            .into_iter()
            .find(|c| c.channel.id == channel.id)
            .ok_or_else(|| BusError::InvalidRequest(format!("channel #{slug} vanished")))
    }

    /// Apaga o canal e as mensagens dele.
    pub async fn delete_channel(&self, team_id: &TeamId, slug: &str) -> BusResult<()> {
        let channel = self.channel(team_id, slug).await?;
        Ok(self.store.delete_channel(&channel.id).await?)
    }

    /// `aisense join #canal` / `leave`: o próprio agente se inscreve ou sai. Entrar num canal
    /// aberto o fecha nele e em quem mais entrar depois — por isso a lista volta na resposta.
    pub async fn subscribe_channel(
        &self,
        me: &Identity,
        slug: &str,
        join: bool,
    ) -> BusResult<ChannelInfo> {
        let slug_clean = slug.trim().trim_start_matches('#');
        let channel = match self.store.channel_by_slug(&me.team_id, slug_clean).await? {
            Some(channel) => channel,
            None if join => {
                return self
                    .save_channel(
                        &me.team_id,
                        slug_clean,
                        "",
                        &[me.handle.as_str().to_owned()],
                    )
                    .await
            }
            None => {
                return Err(BusError::InvalidRequest(format!(
                    "there is no channel #{slug_clean}"
                )))
            }
        };
        let mut members = self.store.channel_members(&channel.id).await?;
        members.retain(|a| a != &me.agent_id);
        if join {
            members.push(me.agent_id.clone());
        }
        self.store
            .set_channel_members(&channel.id, &members)
            .await?;
        self.channels(&me.team_id)
            .await?
            .into_iter()
            .find(|c| c.channel.id == channel.id)
            .ok_or_else(|| BusError::InvalidRequest(format!("channel #{slug_clean} vanished")))
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
    /// Recibos: quantos receberam, quantos já foram entregues (injetados) ou lidos.
    pub receipts: Receipts,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Receipts {
    pub recipients: u32,
    /// Entregues ou lidas.
    pub delivered: u32,
    pub read: u32,
}

impl Receipts {
    pub fn of(deliveries: &[super::model::Delivery]) -> Self {
        let count = |f: &dyn Fn(DeliveryState) -> bool| {
            u32::try_from(deliveries.iter().filter(|d| f(d.state)).count()).unwrap_or(u32::MAX)
        };
        Self {
            recipients: u32::try_from(deliveries.len()).unwrap_or(u32::MAX),
            delivered: count(&|s| matches!(s, DeliveryState::Delivered | DeliveryState::Read)),
            read: count(&|s| s == DeliveryState::Read),
        }
    }
}

impl Directory {
    pub fn view(&self, message: &Message) -> MessageView {
        self.view_with(message, Receipts::default())
    }

    pub fn view_with(&self, message: &Message, receipts: Receipts) -> MessageView {
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
            receipts,
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
        let mut views = Vec::with_capacity(messages.len());
        for m in &messages {
            let receipts = Receipts::of(&self.store.deliveries_of(&m.id).await?);
            views.push(dir.view_with(m, receipts));
        }
        Ok(views)
    }

    /// O evento para a UI de uma mensagem recém-roteada.
    pub async fn event_for(&self, routed: &Routed) -> BusResult<BusMessageEvent> {
        let dir = self.directory(&routed.message.team_id).await?;
        Ok(BusMessageEvent {
            team_id: routed.message.team_id.clone(),
            message: dir.view_with(&routed.message, Receipts::of(&routed.deliveries)),
            recipients: u32::try_from(routed.deliveries.len()).unwrap_or(u32::MAX),
        })
    }
}

impl<S: BusStore> BusService<S> {
    /// `ask`: manda a pergunta e bloqueia até a resposta (`reply_to` = a pergunta) ou o
    /// timeout. Recusa na hora: destinatário parado (`agent_stopped`) e pergunta que fecharia
    /// um ciclo de esperas (`would_deadlock`).
    pub async fn ask(
        &self,
        me: &Identity,
        to: &str,
        body: &str,
        timeout: Option<Duration>,
    ) -> BusResult<Message> {
        let timeout = timeout.unwrap_or(ASK_DEFAULT).min(ASK_MAX);
        let address = Address::parse(to).map_err(BusError::InvalidRequest)?;
        let Address::Agent(handle) = &address else {
            return Err(BusError::InvalidRequest(
                "a question (ask) goes to one agent: aisense ask @alguem \"...\"".into(),
            ));
        };
        let target = self
            .store
            .list_agents(&me.team_id)
            .await?
            .into_iter()
            .find(|a| &a.handle == handle)
            .ok_or_else(|| BusError::UnknownAgent(handle.as_str().to_owned()))?;

        // Registra a espera antes de mandar: dois `ask` cruzados no mesmo instante também
        // são pegos (o segundo vê a aresta do primeiro).
        let edge = (me.agent_id.clone(), target.id.clone());
        let registered = {
            let mut graph = self
                .waits
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match graph.path(&target.id, &me.agent_id) {
                Some(cycle) => Err(cycle),
                None => {
                    graph.edges.push(edge.clone());
                    Ok(Waiting {
                        graph: Arc::clone(&self.waits),
                        edge,
                    })
                }
            }
        };
        let waiting = match registered {
            Ok(waiting) => waiting,
            Err(cycle) => {
                // Nomes montados fora do lock (consulta ao banco).
                let names = match self.directory(&me.team_id).await {
                    Ok(dir) => cycle
                        .iter()
                        .chain(std::iter::once(&target.id))
                        .map(|id| {
                            dir.target(&super::model::Target::Agent {
                                agent_id: id.clone(),
                            })
                        })
                        .collect::<Vec<_>>()
                        .join(" → "),
                    Err(_) => String::new(),
                };
                return Err(BusError::WouldDeadlock {
                    target: handle.as_str().to_owned(),
                    cycle: names,
                });
            }
        };

        let mut rx = self.subscribe();
        let out = Outgoing {
            kind: MessageKind::Request,
            meta: MessageMeta {
                timeout_s: Some(u32::try_from(timeout.as_secs()).unwrap_or(u32::MAX)),
                ..MessageMeta::default()
            },
            ..Outgoing::message(
                Sender::Agent {
                    agent_id: me.agent_id.clone(),
                },
                address.clone(),
                body,
            )
        };
        let question = self.dispatch(&me.team_id, out).await?;
        let question_id = question.message.id.clone();
        let deadline = tokio::time::Instant::now() + timeout;
        let answer = loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Err(_) => {
                    drop(waiting);
                    return Err(BusError::Timeout {
                        handle: handle.as_str().to_owned(),
                        secs: timeout.as_secs(),
                    });
                }
                Ok(Ok(routed)) if routed.message.reply_to.as_ref() == Some(&question_id) => {
                    break routed.message.clone();
                }
                Ok(Ok(_)) => {}
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                    // Perdeu eventos: a resposta pode já estar no banco.
                    if let Some(found) = self
                        .store
                        .replies_to(&question_id)
                        .await?
                        .into_iter()
                        .next()
                    {
                        break found;
                    }
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    return Err(BusError::Timeout {
                        handle: handle.as_str().to_owned(),
                        secs: timeout.as_secs(),
                    })
                }
            }
        };
        drop(waiting);
        // A resposta foi entregue a quem perguntou na própria chamada: conta como lida.
        let _ = self
            .store
            .mark_read(&me.agent_id, std::slice::from_ref(&answer.id), now_ms())
            .await;
        self.observer
            .deliveries_changed(std::slice::from_ref(&answer.id), &me.agent_id);
        Ok(answer)
    }

    /// `reply`: responde uma pergunta (`request`) feita a você. Destrava quem perguntou.
    pub async fn reply(&self, me: &Identity, reply_to: &str, body: &str) -> BusResult<Routed> {
        let id = crate::ids::MessageId::from_raw(reply_to.trim());
        let original = self
            .store
            .get_message(&id)
            .await?
            .filter(|m| m.team_id == me.team_id)
            .ok_or_else(|| BusError::UnknownMessage(id.clone()))?;
        let to_me = self
            .store
            .deliveries_of(&id)
            .await?
            .iter()
            .any(|d| d.agent_id == me.agent_id);
        if !to_me {
            return Err(BusError::InvalidRequest(format!(
                "message {id} was not sent to you; answer only what you received"
            )));
        }
        let address = super::reply_address(&*self.store, &original).await?;
        let kind = if original.kind == MessageKind::Request {
            MessageKind::Response
        } else {
            MessageKind::Message
        };
        let out = Outgoing {
            kind,
            reply_to: Some(id.clone()),
            ..Outgoing::message(
                Sender::Agent {
                    agent_id: me.agent_id.clone(),
                },
                address,
                body,
            )
        };
        let routed = self.dispatch(&me.team_id, out).await?;
        // Respondida = lida.
        if self
            .store
            .mark_read(&me.agent_id, std::slice::from_ref(&id), now_ms())
            .await?
            > 0
        {
            self.observer
                .deliveries_changed(std::slice::from_ref(&id), &me.agent_id);
        }
        Ok(routed)
    }
}
