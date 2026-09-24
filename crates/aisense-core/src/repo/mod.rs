//! Portas de persistência. O core define o contrato; `aisense-store` implementa
//! com SQLite e `InMemoryStore` implementa em memória para os testes do domínio.

mod error;
mod memory;

use std::future::Future;

pub use error::RepoError;
pub use memory::InMemoryStore;

use crate::agent::{Agent, Handle};
use crate::ids::{AgentId, SessionId, SkillId, TeamId};
use crate::skill::{Skill, SkillSource};
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
    /// Tamanho do log no início da sessão: a transcrição dela começa aqui (F03-09).
    /// `None` nas sessões gravadas antes de existir este campo.
    pub log_offset: Option<u64>,
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

/// O dono de um `AISENSE_TOKEN` (tabela `agent_tokens`, invariante I4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRecord {
    pub agent_id: AgentId,
    pub session_id: SessionId,
    pub expires_at: Millis,
}

/// Tokens de IPC: um por sessão de agente, apagados quando ela termina (F05-04).
pub trait TokenRepository: Send + Sync {
    /// Falha com `AgentNotFound`/`Corrupt` se o agente ou a sessão não existem.
    fn insert_token(
        &self,
        token: &str,
        record: &TokenRecord,
    ) -> impl Future<Output = RepoResult<()>> + Send;
    /// Só devolve token ainda não expirado.
    fn find_token(
        &self,
        token: &str,
        now: Millis,
    ) -> impl Future<Output = RepoResult<Option<TokenRecord>>> + Send;
    /// Revoga os tokens de uma sessão (fim do processo). Devolve quantos saíram.
    fn revoke_session_tokens(
        &self,
        session_id: &SessionId,
    ) -> impl Future<Output = RepoResult<u64>> + Send;
    /// Revoga tudo — na subida do app, nenhuma sessão anterior está viva.
    fn revoke_all_tokens(&self) -> impl Future<Output = RepoResult<u64>> + Send;
}

/// Uma skill da biblioteca como o banco a conhece (tabela `skills`). O conteúdo mora no
/// disco (`SKILL.md`); aqui fica a identidade estável que as atribuições referenciam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRecord {
    pub id: SkillId,
    /// O `name` do frontmatter.
    pub slug: String,
    pub description: String,
    pub version: String,
    /// `builtin` ou `user`.
    pub source: String,
    /// Pasta da skill; `builtin:<nome>` para as embutidas.
    pub path: String,
    pub targets: Vec<String>,
    pub created_at: Millis,
    pub updated_at: Millis,
}

impl SkillRecord {
    /// Colunas que vêm do `SKILL.md`, para comparar com o banco sem olhar datas e id.
    pub fn matches(&self, skill: &Skill) -> bool {
        let (source, path) = skill_origin(skill);
        self.description == skill.description
            && self.version == skill.version
            && self.source == source
            && self.path == path
            && self.targets == skill.targets
    }
}

/// `(source, path)` como vão para a tabela `skills`.
pub fn skill_origin(skill: &Skill) -> (&'static str, String) {
    match &skill.source {
        SkillSource::Builtin => ("builtin", format!("builtin:{}", skill.name)),
        SkillSource::User { dir } => ("user", dir.clone()),
    }
}

/// Uma skill atribuída a um agente (tabela `agent_skills`), na ordem de injeção.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentSkill {
    pub skill_id: SkillId,
    pub enabled: bool,
}

pub trait SkillRepository: Send + Sync {
    /// Espelha a biblioteca do disco no banco: insere as novas, atualiza as que mudaram
    /// (casando por `slug`). **Não apaga** as que sumiram do disco — um `SKILL.md` com erro
    /// de digitação não pode levar embora as atribuições. Devolve todas, por `slug`.
    fn sync_skills(
        &self,
        skills: &[Skill],
        now: Millis,
    ) -> impl Future<Output = RepoResult<Vec<SkillRecord>>> + Send;
    /// Todas as conhecidas, por `slug`.
    fn list_skills(&self) -> impl Future<Output = RepoResult<Vec<SkillRecord>>> + Send;
    /// As skills do agente na ordem de injeção.
    fn agent_skills(
        &self,
        agent_id: &AgentId,
    ) -> impl Future<Output = RepoResult<Vec<AgentSkill>>> + Send;
    /// Troca a lista inteira de uma vez; a posição é a ordem de `skills`. Falha com
    /// `AgentNotFound` ou `SkillNotFound` sem mudar nada. Repetida vale a primeira.
    fn set_agent_skills(
        &self,
        agent_id: &AgentId,
        skills: &[AgentSkill],
    ) -> impl Future<Output = RepoResult<()>> + Send;
    /// Quem usa a skill (habilitada ou não) — o aviso de "N agentes precisam reiniciar".
    fn skill_users(
        &self,
        skill_id: &SkillId,
    ) -> impl Future<Output = RepoResult<Vec<AgentId>>> + Send;
}

/// Primeira ocorrência de cada skill, na ordem dada.
pub fn dedup_agent_skills(skills: &[AgentSkill]) -> Vec<AgentSkill> {
    let mut seen = std::collections::HashSet::new();
    skills
        .iter()
        .filter(|s| seen.insert(s.skill_id.clone()))
        .cloned()
        .collect()
}
