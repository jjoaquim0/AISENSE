//! Propostas no app (F07-02): o mesmo `ProposalService` que o socket usa; a UI lista as
//! pendentes (`proposal:changed`) e você aceita ou recusa.

use std::sync::Arc;

use aisense_core::proposal::{Proposal, ProposalError, ProposalObserver, ProposalService};
use aisense_core::{CommandError, ProposalId, TeamId};
use aisense_store::Store;
use tauri::{AppHandle, Emitter, State};

use super::bus::Bus;

pub type Proposals = ProposalService<Store>;

/// Payload: `Proposal` (nova ou decidida).
pub const PROPOSAL_CHANGED: &str = "proposal:changed";

struct TauriProposalObserver {
    app: AppHandle,
}

impl ProposalObserver for TauriProposalObserver {
    fn changed(&self, proposal: &Proposal) {
        if let Err(error) = self.app.emit(PROPOSAL_CHANGED, proposal) {
            tracing::warn!(%error, "falha ao avisar a UI sobre proposta");
        }
    }
}

pub fn setup(app: &AppHandle, bus: &Bus) -> Proposals {
    ProposalService::new(
        bus.clone(),
        Arc::new(TauriProposalObserver { app: app.clone() }),
    )
}

fn proposal_error(error: ProposalError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), error.hint())
}

#[tauri::command]
pub async fn proposals_list(
    proposals: State<'_, Proposals>,
    team_id: TeamId,
    pending_only: bool,
) -> Result<Vec<Proposal>, CommandError> {
    proposals
        .list(&team_id, pending_only)
        .await
        .map_err(proposal_error)
}

/// Aceitar executa (criar agente, autonomia) ou registra que você vai editar (skill, colunas).
#[tauri::command]
pub async fn proposal_decide(
    proposals: State<'_, Proposals>,
    team_id: TeamId,
    id: ProposalId,
    accept: bool,
    note: Option<String>,
) -> Result<Proposal, CommandError> {
    proposals
        .decide(&team_id, &id, accept, note.as_deref())
        .await
        .map_err(proposal_error)
}
