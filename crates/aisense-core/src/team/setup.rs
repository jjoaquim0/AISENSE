//! Operações de equipe que envolvem mais de uma entidade (`docs/09`, T2 e T3).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Team;
use super::TeamDraft;
use crate::agent::{Agent, AgentDraft, AgentState, Handle};
use crate::color::AgentColor;
use crate::ids::AgentId;
use crate::repo::{AgentRepository, RepoError, TeamRepository};
use crate::time::Millis;
use crate::validation::ValidationError;

#[derive(Debug, thiserror::Error)]
pub enum TeamSetupError {
    /// `index` é a posição do agente na lista enviada; `None` = erro na equipe.
    #[error("{}{source}", index.map(|i| format!("agent #{}: ", i + 1)).unwrap_or_default())]
    Invalid {
        index: Option<usize>,
        source: ValidationError,
    },
    #[error("the typed name does not match the team name")]
    NameMismatch,
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl TeamSetupError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid { source, .. } => source.code(),
            Self::NameMismatch => "name_mismatch",
            Self::Repo(e) => e.code(),
        }
    }
}

/// Cria a equipe e seus agentes de uma vez. Tudo é validado antes de gravar; se um
/// agente falhar na gravação, a equipe é desfeita — não sobra equipe pela metade.
pub async fn create_team_with_agents<S: TeamRepository + AgentRepository>(
    store: &S,
    draft: &TeamDraft,
    agents: &[AgentDraft],
    now: Millis,
) -> Result<(Team, Vec<Agent>), TeamSetupError> {
    let team = Team::create(draft, now).map_err(|source| TeamSetupError::Invalid {
        index: None,
        source,
    })?;
    let mut created: Vec<Agent> = Vec::with_capacity(agents.len());
    for (index, agent_draft) in agents.iter().enumerate() {
        let agent =
            Agent::create(team.id.clone(), agent_draft, &created, now).map_err(|source| {
                TeamSetupError::Invalid {
                    index: Some(index),
                    source,
                }
            })?;
        created.push(agent);
    }

    store.create_team(&team).await?;
    for agent in &created {
        if let Err(error) = store.create_agent(agent).await {
            if let Err(cleanup) = store.delete_team(&team.id).await {
                tracing::warn!(team = %team.id, %cleanup, "equipe parcial não foi desfeita");
            }
            return Err(error.into());
        }
    }
    Ok((team, created))
}

/// Excluir apaga tudo o que é da equipe (I5), então exige digitar o nome (`docs/09`).
pub fn confirm_deletion(team: &Team, typed: &str) -> Result<(), TeamSetupError> {
    if typed.trim() == team.name {
        Ok(())
    } else {
        Err(TeamSetupError::NameMismatch)
    }
}

/// O que um card da tela de equipes (T2) mostra de cada agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentSummary {
    pub id: AgentId,
    pub handle: Handle,
    pub name: String,
    pub color: AgentColor,
    pub adapter_id: String,
    pub autostart: bool,
    pub state: AgentState,
}

impl AgentSummary {
    pub fn new(agent: &Agent, state: AgentState) -> Self {
        Self {
            id: agent.id.clone(),
            handle: agent.handle.clone(),
            name: agent.name.clone(),
            color: agent.color,
            adapter_id: agent.adapter_id.clone(),
            autostart: agent.autostart,
            state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct TeamSummary {
    pub team: Team,
    pub agents: Vec<AgentSummary>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::repo::InMemoryStore;
    use crate::team::TeamTemplate;

    fn team_draft(name: &str) -> TeamDraft {
        TeamDraft {
            name: name.into(),
            workdir: "/home/dev/app".into(),
            ..TeamDraft::default()
        }
    }

    fn drafts(template: TeamTemplate) -> Vec<AgentDraft> {
        template
            .plan(|_| true)
            .into_iter()
            .map(|p| p.draft)
            .collect()
    }

    #[tokio::test]
    async fn duo_dev_template_creates_both_agents() {
        let store = InMemoryStore::new();
        let (team, agents) = create_team_with_agents(
            &store,
            &team_draft("Dupla"),
            &drafts(TeamTemplate::DuoDev),
            1,
        )
        .await
        .unwrap();

        let stored = store.list_agents(&team.id).await.unwrap();
        assert_eq!(stored, agents);
        let handles: Vec<&str> = stored.iter().map(|a| a.handle.as_str()).collect();
        assert_eq!(handles, vec!["dev", "revisor"]);
        assert_ne!(stored[0].color, stored[1].color);
    }

    #[tokio::test]
    async fn an_invalid_agent_writes_nothing() {
        let store = InMemoryStore::new();
        let mut agents = drafts(TeamTemplate::DuoDev);
        agents[1].handle = "all".into(); // reservado
        let error = create_team_with_agents(&store, &team_draft("Dupla"), &agents, 1)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            TeamSetupError::Invalid { index: Some(1), .. }
        ));
        assert!(error.to_string().starts_with("agent #2"), "{error}");
        assert!(store
            .list_teams(crate::repo::TeamFilter {
                include_archived: true
            })
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn duplicate_handles_in_the_request_are_refused() {
        let store = InMemoryStore::new();
        let mut agents = drafts(TeamTemplate::DuoDev);
        agents[1].handle = "dev".into();
        let error = create_team_with_agents(&store, &team_draft("Dupla"), &agents, 1)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "duplicate_handle");
    }

    #[test]
    fn deletion_needs_the_exact_name() {
        let team = Team::create(&team_draft("Squad Produto"), 1).unwrap();
        assert!(confirm_deletion(&team, " Squad Produto ").is_ok());
        assert!(matches!(
            confirm_deletion(&team, "squad produto"),
            Err(TeamSetupError::NameMismatch)
        ));
    }
}
