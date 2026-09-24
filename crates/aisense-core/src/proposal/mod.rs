//! Propostas (`docs/11`, "Comandos do barramento que exigem confirmação humana"; F07-02).
//!
//! Nenhum agente cria, apaga ou reconfigura outro agente, muda autonomia, edita skill ou altera
//! as colunas do quadro sozinho. Ele **propõe**; a proposta aparece na UI com Aceitar/Recusar e
//! só você executa. Agente que cria agente é recursão sem caso base.

#[cfg(test)]
mod tests;

use std::future::Future;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agent::{create_agent, update_agent, AgentDraft, Autonomy, Handle};
use crate::bus::{Address, BusService, BusStore, Identity, MessageKind, Outgoing, Sender};
use crate::ids::{AgentId, ProposalId, TeamId};
use crate::repo::{RepoError, RepoResult};
use crate::time::{now_ms, Millis};

/// O que o agente quer que aconteça.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum ProposalAction {
    /// Um agente novo na equipe.
    #[serde(rename_all = "camelCase")]
    CreateAgent {
        handle: String,
        name: String,
        role: String,
        adapter_id: String,
    },
    /// Mudar a autonomia de um agente.
    #[serde(rename_all = "camelCase")]
    SetAutonomy { handle: String, autonomy: Autonomy },
    /// Mudar uma skill (o humano edita; a proposta diz o quê).
    #[serde(rename_all = "camelCase")]
    EditSkill { skill: String, change: String },
    /// Mudar as colunas do quadro (o humano edita no editor de colunas).
    #[serde(rename_all = "camelCase")]
    ChangeColumns { change: String },
}

impl ProposalAction {
    /// Uma linha para a UI e para a mensagem ao humano.
    pub fn describe(&self) -> String {
        match self {
            Self::CreateAgent {
                handle, adapter_id, ..
            } => format!(
                "criar o agente @{} ({adapter_id})",
                handle.trim_start_matches('@')
            ),
            Self::SetAutonomy { handle, autonomy } => format!(
                "mudar a autonomia de @{} para {}",
                handle.trim_start_matches('@'),
                autonomy.as_str()
            ),
            Self::EditSkill { skill, change } => format!("editar a skill {skill}: {change}"),
            Self::ChangeColumns { change } => format!("mudar as colunas do quadro: {change}"),
        }
    }

    /// Aceitar executa sozinho (criar agente, autonomia) ou só registra que você concorda e
    /// vai fazer a edição (skill, colunas).
    pub fn executes(&self) -> bool {
        matches!(self, Self::CreateAgent { .. } | Self::SetAutonomy { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum ProposalState {
    Pending,
    Accepted,
    Rejected,
}

impl ProposalState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
// `Proposal` já é o TOML sugerido do `aisense.toml` (project::detect) no TypeScript.
#[ts(
    export,
    rename = "AgentProposal",
    export_to = "../../../apps/desktop/src/types/generated/"
)]
pub struct Proposal {
    pub id: ProposalId,
    pub team_id: TeamId,
    /// `None` se o agente foi removido depois.
    pub proposed_by: Option<AgentId>,
    pub action: ProposalAction,
    pub reason: String,
    pub state: ProposalState,
    #[ts(type = "number")]
    pub created_at: Millis,
    #[ts(type = "number | null")]
    pub decided_at: Option<Millis>,
    pub decision_note: Option<String>,
}

pub trait ProposalRepository: Send + Sync {
    fn insert_proposal(&self, proposal: &Proposal) -> impl Future<Output = RepoResult<()>> + Send;
    fn get_proposal(
        &self,
        id: &ProposalId,
    ) -> impl Future<Output = RepoResult<Option<Proposal>>> + Send;
    /// Mais recente primeiro.
    fn list_proposals(
        &self,
        team_id: &TeamId,
        pending_only: bool,
    ) -> impl Future<Output = RepoResult<Vec<Proposal>>> + Send;
    /// Decide só se ainda estiver pendente; `false` se alguém decidiu antes.
    fn decide_proposal(
        &self,
        id: &ProposalId,
        state: ProposalState,
        now: Millis,
        note: Option<&str>,
    ) -> impl Future<Output = RepoResult<bool>> + Send;
}

pub trait ProposalStore: ProposalRepository + BusStore {}
impl<T: ProposalRepository + BusStore> ProposalStore for T {}

#[derive(Debug, thiserror::Error)]
pub enum ProposalError {
    /// A resposta normal a um agente que propõe: a ação espera você.
    #[error(
        "{} precisa do humano: a proposta {id} está na UI com Aceitar/Recusar.",
        action
    )]
    NeedsApproval { id: ProposalId, action: String },
    #[error("{0}")]
    Invalid(String),
    #[error("A proposta {0} não existe nesta equipe.")]
    Unknown(String),
    #[error("A proposta {0} já foi decidida.")]
    AlreadyDecided(ProposalId),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl ProposalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NeedsApproval { .. } => "needs_approval",
            Self::Invalid(_) => "invalid_request",
            Self::Unknown(_) => "unknown_proposal",
            Self::AlreadyDecided(_) => "already_decided",
            Self::Repo(_) => "internal",
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Self::NeedsApproval { .. } => Some(
                "Siga com outra coisa; você recebe uma mensagem quando o humano decidir.".into(),
            ),
            _ => None,
        }
    }
}

/// Quem mais precisa saber (o app emite `proposal:changed`).
pub trait ProposalObserver: Send + Sync + 'static {
    fn changed(&self, _proposal: &Proposal) {}
}

pub struct NoProposalObserver;
impl ProposalObserver for NoProposalObserver {}

pub struct ProposalService<S> {
    bus: BusService<S>,
    observer: Arc<dyn ProposalObserver>,
}

impl<S> Clone for ProposalService<S> {
    fn clone(&self) -> Self {
        Self {
            bus: self.bus.clone(),
            observer: Arc::clone(&self.observer),
        }
    }
}

impl<S: ProposalStore> ProposalService<S> {
    pub fn new(bus: BusService<S>, observer: Arc<dyn ProposalObserver>) -> Self {
        Self { bus, observer }
    }

    fn store(&self) -> &S {
        self.bus.store()
    }

    async fn notify(&self, team_id: &TeamId, to: Address, body: String) {
        let out = Outgoing {
            kind: MessageKind::System,
            subject: Some("proposta".into()),
            ..Outgoing::message(Sender::System, to, body)
        };
        if let Err(error) = self.bus.dispatch(team_id, out).await {
            tracing::warn!(%error, "aviso de proposta não entregue");
        }
    }

    async fn check(&self, team_id: &TeamId, action: &ProposalAction) -> Result<(), ProposalError> {
        let agents = self.store().list_agents(team_id).await?;
        let find = |raw: &str| {
            let name = raw.trim().trim_start_matches('@');
            agents.iter().find(|a| a.handle.as_str() == name)
        };
        match action {
            ProposalAction::CreateAgent {
                handle,
                name,
                adapter_id,
                ..
            } => {
                let handle = Handle::parse(handle.trim().trim_start_matches('@'))
                    .map_err(|e| ProposalError::Invalid(format!("handle inválido: {e}")))?;
                if find(handle.as_str()).is_some() {
                    return Err(ProposalError::Invalid(format!(
                        "@{} já existe nesta equipe.",
                        handle.as_str()
                    )));
                }
                if name.trim().is_empty() || adapter_id.trim().is_empty() {
                    return Err(ProposalError::Invalid(
                        "Diga o nome e o runtime do agente novo (--runtime claude).".into(),
                    ));
                }
            }
            ProposalAction::SetAutonomy { handle, .. } => {
                if find(handle).is_none() {
                    return Err(ProposalError::Invalid(format!(
                        "Não há agente {handle} nesta equipe."
                    )));
                }
            }
            ProposalAction::EditSkill { skill, change } => {
                if skill.trim().is_empty() || change.trim().is_empty() {
                    return Err(ProposalError::Invalid(
                        "Diga a skill e o que mudar nela.".into(),
                    ));
                }
            }
            ProposalAction::ChangeColumns { change } => {
                if change.trim().is_empty() {
                    return Err(ProposalError::Invalid(
                        "Diga o que mudar nas colunas.".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Um agente propõe. A resposta é sempre `NeedsApproval` (com o id): a ação não
    /// acontece, ela espera você na UI.
    pub async fn propose(
        &self,
        me: &Identity,
        action: ProposalAction,
        reason: &str,
    ) -> Result<Proposal, ProposalError> {
        if reason.trim().is_empty() {
            return Err(ProposalError::Invalid(
                "Explique por quê (--reason): é o que o humano lê para decidir.".into(),
            ));
        }
        self.check(&me.team_id, &action).await?;
        let proposal = Proposal {
            id: ProposalId::new(),
            team_id: me.team_id.clone(),
            proposed_by: Some(me.agent_id.clone()),
            action,
            reason: reason.trim().to_owned(),
            state: ProposalState::Pending,
            created_at: now_ms(),
            decided_at: None,
            decision_note: None,
        };
        self.store().insert_proposal(&proposal).await?;
        self.notify(
            &me.team_id,
            Address::Human,
            format!(
                "@{} propõe {}. Motivo: {}\nAceite ou recuse na UI ({}).",
                me.handle.as_str(),
                proposal.action.describe(),
                proposal.reason,
                proposal.id
            ),
        )
        .await;
        self.observer.changed(&proposal);
        Ok(proposal)
    }

    pub async fn list(
        &self,
        team_id: &TeamId,
        pending_only: bool,
    ) -> Result<Vec<Proposal>, ProposalError> {
        Ok(self.store().list_proposals(team_id, pending_only).await?)
    }

    /// Você decide. Aceitar executa o que dá para executar (criar agente, autonomia); o
    /// resto fica registrado para você fazer no editor. Quem propôs recebe a decisão.
    pub async fn decide(
        &self,
        team_id: &TeamId,
        id: &ProposalId,
        accept: bool,
        note: Option<&str>,
    ) -> Result<Proposal, ProposalError> {
        let proposal = self
            .store()
            .get_proposal(id)
            .await?
            .filter(|p| &p.team_id == team_id)
            .ok_or_else(|| ProposalError::Unknown(id.to_string()))?;
        if proposal.state != ProposalState::Pending {
            return Err(ProposalError::AlreadyDecided(id.clone()));
        }
        if accept {
            self.check(team_id, &proposal.action).await?;
            self.execute(team_id, &proposal.action).await?;
        }
        let state = if accept {
            ProposalState::Accepted
        } else {
            ProposalState::Rejected
        };
        let note = note.map(str::trim).filter(|n| !n.is_empty());
        if !self
            .store()
            .decide_proposal(id, state, now_ms(), note)
            .await?
        {
            return Err(ProposalError::AlreadyDecided(id.clone()));
        }
        let decided = self
            .store()
            .get_proposal(id)
            .await?
            .ok_or_else(|| ProposalError::Unknown(id.to_string()))?;
        if let Some(agent) = &decided.proposed_by {
            if let Some(agent) = self.store().get_agent(agent).await? {
                let verdict = match (accept, decided.action.executes()) {
                    (true, true) => "aceitou e aplicou",
                    (true, false) => "aceitou (vai fazer a mudança)",
                    (false, _) => "recusou",
                };
                let mut body = format!(
                    "O humano {verdict} sua proposta de {}.",
                    decided.action.describe()
                );
                if let Some(note) = &decided.decision_note {
                    body.push_str(&format!(" Nota: {note}"));
                }
                self.notify(team_id, Address::Agent(agent.handle), body)
                    .await;
            }
        }
        self.observer.changed(&decided);
        Ok(decided)
    }

    async fn execute(
        &self,
        team_id: &TeamId,
        action: &ProposalAction,
    ) -> Result<(), ProposalError> {
        let invalid = |e: crate::agent::AgentOpError| ProposalError::Invalid(e.to_string());
        match action {
            ProposalAction::CreateAgent {
                handle,
                name,
                role,
                adapter_id,
            } => {
                let draft = AgentDraft {
                    handle: handle.trim().trim_start_matches('@').to_owned(),
                    name: name.trim().to_owned(),
                    role: role.trim().to_owned(),
                    adapter_id: adapter_id.trim().to_owned(),
                    ..AgentDraft::default()
                };
                create_agent(self.store(), team_id, &draft, now_ms())
                    .await
                    .map_err(invalid)?;
            }
            ProposalAction::SetAutonomy { handle, autonomy } => {
                let name = handle.trim().trim_start_matches('@');
                let agent = self
                    .store()
                    .list_agents(team_id)
                    .await?
                    .into_iter()
                    .find(|a| a.handle.as_str() == name)
                    .ok_or_else(|| ProposalError::Invalid(format!("Não há agente @{name}.")))?;
                let draft = AgentDraft {
                    autonomy: *autonomy,
                    ..agent.to_draft()
                };
                update_agent(self.store(), &agent.id, &draft, false, now_ms())
                    .await
                    .map_err(invalid)?;
            }
            ProposalAction::EditSkill { .. } | ProposalAction::ChangeColumns { .. } => {}
        }
        Ok(())
    }
}
