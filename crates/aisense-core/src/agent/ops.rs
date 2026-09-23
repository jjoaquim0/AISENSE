//! Criar e editar um agente avulso (`docs/09`, T5), validando contra a equipe.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Agent, AgentDraft};
use crate::ids::{AgentId, TeamId};
use crate::repo::{AgentRepository, RepoError, TeamRepository};
use crate::time::Millis;
use crate::validation::ValidationError;

#[derive(Debug, thiserror::Error)]
pub enum AgentOpError {
    #[error(transparent)]
    Invalid(#[from] ValidationError),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl AgentOpError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(e) => e.code(),
            Self::Repo(e) => e.code(),
        }
    }
}

/// Resultado de uma edição. Mudar um agente vivo não mexe no processo: o que mudou
/// (runtime, ambiente, argumentos...) só vale a partir do próximo início.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentUpdate {
    pub agent: Agent,
    pub restart_required: bool,
}

pub async fn create_agent<S: TeamRepository + AgentRepository>(
    store: &S,
    team_id: &TeamId,
    draft: &AgentDraft,
    now: Millis,
) -> Result<Agent, AgentOpError> {
    if store.get_team(team_id).await?.is_none() {
        return Err(RepoError::TeamNotFound(team_id.clone()).into());
    }
    let siblings = store.list_agents(team_id).await?;
    let agent = Agent::create(team_id.clone(), draft, &siblings, now)?;
    store.create_agent(&agent).await?;
    Ok(agent)
}

/// `running` diz se o agente está com processo vivo agora; decide o aviso de reinício.
pub async fn update_agent<S: AgentRepository>(
    store: &S,
    agent_id: &AgentId,
    draft: &AgentDraft,
    running: bool,
    now: Millis,
) -> Result<AgentUpdate, AgentOpError> {
    let mut agent = store
        .get_agent(agent_id)
        .await?
        .ok_or_else(|| RepoError::AgentNotFound(agent_id.clone()))?;
    let before = agent.clone();
    let siblings = store.list_agents(&agent.team_id).await?;
    agent.apply(draft, &siblings, now)?;
    store.update_agent(&agent).await?;
    let restart_required = running && affects_the_process(&before, &agent);
    Ok(AgentUpdate {
        agent,
        restart_required,
    })
}

/// Campos lidos só na hora de subir o processo. Nome, cor e posição não pedem reinício.
fn affects_the_process(before: &Agent, after: &Agent) -> bool {
    before.handle != after.handle
        || before.role != after.role
        || before.adapter_id != after.adapter_id
        || before.model != after.model
        || before.workdir != after.workdir
        || before.env != after.env
        || before.args != after.args
        || before.delivery_mode != after.delivery_mode
        || before.autonomy != after.autonomy
        || before.workbench != after.workbench
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::repo::InMemoryStore;
    use crate::team::{Team, TeamDraft};

    async fn team(store: &InMemoryStore) -> Team {
        let team = Team::create(
            &TeamDraft {
                name: "Squad".into(),
                workdir: "/tmp".into(),
                ..TeamDraft::default()
            },
            1,
        )
        .unwrap();
        store.create_team(&team).await.unwrap();
        team
    }

    fn draft(handle: &str) -> AgentDraft {
        AgentDraft {
            handle: handle.into(),
            name: handle.to_uppercase(),
            adapter_id: "claude".into(),
            ..AgentDraft::default()
        }
    }

    #[tokio::test]
    async fn duplicate_handle_is_refused_before_writing() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        create_agent(&store, &t.id, &draft("backend"), 1)
            .await
            .unwrap();
        let error = create_agent(&store, &t.id, &draft("backend"), 2)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "duplicate_handle");
        assert_eq!(store.list_agents(&t.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn editing_a_running_agent_asks_for_restart_only_when_it_matters() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        let agent = create_agent(&store, &t.id, &draft("backend"), 1)
            .await
            .unwrap();

        let mut rename = agent.to_draft();
        rename.name = "Back".into();
        let update = update_agent(&store, &agent.id, &rename, true, 2)
            .await
            .unwrap();
        assert!(!update.restart_required, "nome não muda o processo");

        let mut runtime = update.agent.to_draft();
        runtime.adapter_id = "codex".into();
        let update = update_agent(&store, &agent.id, &runtime, true, 3)
            .await
            .unwrap();
        assert!(update.restart_required);

        let mut stopped = update.agent.to_draft();
        stopped.adapter_id = "opencode".into();
        let update = update_agent(&store, &agent.id, &stopped, false, 4)
            .await
            .unwrap();
        assert!(!update.restart_required, "parado não precisa reiniciar");
        assert_eq!(update.agent.adapter_id, "opencode");
    }

    #[tokio::test]
    async fn renaming_onto_a_sibling_handle_is_refused() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        create_agent(&store, &t.id, &draft("backend"), 1)
            .await
            .unwrap();
        let front = create_agent(&store, &t.id, &draft("frontend"), 1)
            .await
            .unwrap();
        let mut clash = front.to_draft();
        clash.handle = "backend".into();
        let error = update_agent(&store, &front.id, &clash, false, 2)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "duplicate_handle");
    }

    #[tokio::test]
    async fn unknown_team_is_reported() {
        let store = InMemoryStore::new();
        let error = create_agent(&store, &TeamId::new(), &draft("backend"), 1)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "team_not_found");
    }
}
