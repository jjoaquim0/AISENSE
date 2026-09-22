//! Portas de persistência. O core define o contrato; `aisense-store` implementa
//! com SQLite e `InMemoryStore` implementa em memória para os testes do domínio.

mod error;
mod memory;

use std::future::Future;

pub use error::RepoError;
pub use memory::InMemoryStore;

use crate::agent::{Agent, Handle};
use crate::ids::{AgentId, TeamId};
use crate::team::Team;
use crate::time::Millis;

pub type RepoResult<T> = Result<T, RepoError>;

/// Filtro da listagem de equipes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TeamFilter {
    pub include_archived: bool,
}

pub trait TeamRepository: Send + Sync {
    fn create_team(&self, team: &Team) -> impl Future<Output = RepoResult<()>> + Send;
    fn get_team(&self, id: &TeamId) -> impl Future<Output = RepoResult<Option<Team>>> + Send;
    /// Ordenadas por criação (mais antiga primeiro).
    fn list_teams(&self, filter: TeamFilter) -> impl Future<Output = RepoResult<Vec<Team>>> + Send;
    fn update_team(&self, team: &Team) -> impl Future<Output = RepoResult<()>> + Send;
    /// `Some(instante)` arquiva, `None` desarquiva. Agentes são preservados.
    fn set_team_archived(
        &self,
        id: &TeamId,
        archived_at: Option<Millis>,
    ) -> impl Future<Output = RepoResult<()>> + Send;
    /// Apaga a equipe e, em cascata, tudo que é dela (I5).
    fn delete_team(&self, id: &TeamId) -> impl Future<Output = RepoResult<()>> + Send;
}

pub trait AgentRepository: Send + Sync {
    /// Falha com `TeamNotFound` se a equipe não existe e `DuplicateHandle` se o
    /// handle já é usado na equipe (I1) — a validação do core pode ter corrido
    /// contra uma lista desatualizada, então a porta também garante.
    fn create_agent(&self, agent: &Agent) -> impl Future<Output = RepoResult<()>> + Send;
    fn get_agent(&self, id: &AgentId) -> impl Future<Output = RepoResult<Option<Agent>>> + Send;
    fn find_agent_by_handle(
        &self,
        team_id: &TeamId,
        handle: &Handle,
    ) -> impl Future<Output = RepoResult<Option<Agent>>> + Send;
    /// Ordenados por `position`, depois por criação.
    fn list_agents(&self, team_id: &TeamId) -> impl Future<Output = RepoResult<Vec<Agent>>> + Send;
    fn update_agent(&self, agent: &Agent) -> impl Future<Output = RepoResult<()>> + Send;
    fn delete_agent(&self, id: &AgentId) -> impl Future<Output = RepoResult<()>> + Send;
}
