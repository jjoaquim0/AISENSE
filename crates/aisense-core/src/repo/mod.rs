//! Portas de persistência. O core define o contrato; `aisense-store` implementa
//! com SQLite e `InMemoryStore` implementa em memória para os testes do domínio.

mod error;
mod memory;

use std::future::Future;

pub use error::RepoError;
pub use memory::InMemoryStore;

use crate::agent::{Agent, Handle};
use crate::ids::{AgentId, SessionId, TeamId};
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

/// Quantas execuções guardar por agente (`docs/04`, retenção de `sessions`).
pub const SESSIONS_KEPT_PER_AGENT: usize = 200;

/// Uma execução de PTY de um agente (tabela `sessions`). O buffer vivo não fica aqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: SessionId,
    pub agent_id: AgentId,
    pub pid: Option<u32>,
    pub started_at: Millis,
    pub ended_at: Option<Millis>,
    pub exit_code: Option<i32>,
    pub log_path: String,
}

pub trait SessionRepository: Send + Sync {
    /// Registra o início e poda o histórico do agente para
    /// [`SESSIONS_KEPT_PER_AGENT`]. Falha com `AgentNotFound` se o agente não existe.
    fn start_session(&self, session: &SessionRecord)
        -> impl Future<Output = RepoResult<()>> + Send;
    /// Marca o fim. Sessão desconhecida (já podada) não é erro.
    fn end_session(
        &self,
        id: &SessionId,
        ended_at: Millis,
        exit_code: Option<i32>,
    ) -> impl Future<Output = RepoResult<()>> + Send;
    /// Mais recente primeiro.
    fn list_sessions(
        &self,
        agent_id: &AgentId,
        limit: usize,
    ) -> impl Future<Output = RepoResult<Vec<SessionRecord>>> + Send;
}
