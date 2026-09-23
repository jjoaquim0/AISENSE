use crate::ids::{AgentId, SkillId, TeamId};

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("team {0} not found")]
    TeamNotFound(TeamId),
    #[error("agent {0} not found")]
    AgentNotFound(AgentId),
    #[error("skill {0} not found")]
    SkillNotFound(SkillId),
    #[error("handle @{0} is already taken in this team")]
    DuplicateHandle(String),
    #[error("{0} already exists")]
    AlreadyExists(String),
    /// Dado no banco que não passa pela validação do domínio (edição manual, bug antigo).
    #[error("corrupt record: {0}")]
    Corrupt(String),
    #[error("storage failure: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl RepoError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TeamNotFound(_) => "team_not_found",
            Self::AgentNotFound(_) => "agent_not_found",
            Self::SkillNotFound(_) => "skill_not_found",
            Self::DuplicateHandle(_) => "duplicate_handle",
            Self::AlreadyExists(_) => "already_exists",
            Self::Corrupt(_) => "corrupt_record",
            Self::Backend(_) => "storage_failure",
        }
    }
}
