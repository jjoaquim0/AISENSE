//! Criar e editar um agente avulso (`docs/09`, T5), validando contra a equipe.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model::AGENT_NAME_MAX;
use super::{Agent, AgentDraft, HANDLE_MAX_LEN};
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

/// "Duplicar" do menu `⋮` (`docs/09`, T4.1): mesma configuração, handle livre
/// (`backend` → `backend-2`, `backend-3`...), nome com "(cópia)" e a próxima cor livre
/// da equipe, para as duas não se confundirem na tela.
pub async fn duplicate_agent<S: TeamRepository + AgentRepository>(
    store: &S,
    agent_id: &AgentId,
    now: Millis,
) -> Result<Agent, AgentOpError> {
    let original = store
        .get_agent(agent_id)
        .await?
        .ok_or_else(|| RepoError::AgentNotFound(agent_id.clone()))?;
    let siblings = store.list_agents(&original.team_id).await?;
    let mut draft = original.to_draft();
    draft.handle = free_handle(original.handle.as_str(), &siblings);
    draft.name = copy_name(&original.name);
    draft.color = None;
    create_agent(store, &original.team_id, &draft, now).await
}

fn free_handle(base: &str, siblings: &[Agent]) -> String {
    let taken = |h: &str| siblings.iter().any(|a| a.handle.as_str() == h);
    (2u32..)
        .map(|n| {
            let suffix = format!("-{n}");
            // Cabe em 32 caracteres cortando a base, nunca o sufixo.
            let keep = HANDLE_MAX_LEN.saturating_sub(suffix.len());
            let base: String = base.chars().take(keep).collect();
            format!("{}{suffix}", base.trim_end_matches('-'))
        })
        .find(|h| !taken(h))
        .unwrap_or_else(|| base.to_owned())
}

fn copy_name(name: &str) -> String {
    const SUFFIX: &str = " (cópia)";
    let keep = AGENT_NAME_MAX.saturating_sub(SUFFIX.chars().count());
    let base: String = name.chars().take(keep).collect();
    format!("{}{SUFFIX}", base.trim_end())
}

/// Reordenação da sidebar (F03-07). A ordem é a da equipe inteira — também é a ordem em
/// que ▶ sobe os agentes —, então precisa listar cada agente exatamente uma vez; uma
/// lista velha (agente criado ou excluído no meio do arraste) é recusada, não remendada.
/// Só grava quem mudou de lugar, e posição não mexe em `updated_at`, como no layout.
pub async fn reorder_agents<S: AgentRepository>(
    store: &S,
    team_id: &TeamId,
    order: &[AgentId],
) -> Result<Vec<Agent>, AgentOpError> {
    let current = store.list_agents(team_id).await?;
    let unique: std::collections::HashSet<&AgentId> = order.iter().collect();
    let complete = unique.len() == order.len()
        && order.len() == current.len()
        && current.iter().all(|a| unique.contains(&a.id));
    if !complete {
        return Err(ValidationError::InvalidOrder.into());
    }
    let mut reordered = Vec::with_capacity(order.len());
    for (index, id) in order.iter().enumerate() {
        let Some(mut agent) = current.iter().find(|a| &a.id == id).cloned() else {
            return Err(ValidationError::InvalidOrder.into());
        };
        let position = i32::try_from(index).map_err(|_| ValidationError::InvalidOrder)?;
        if agent.position != position {
            agent.position = position;
            store.update_agent(&agent).await?;
        }
        reordered.push(agent);
    }
    Ok(reordered)
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

    #[tokio::test]
    async fn duplicate_copies_the_setup_with_a_free_handle_and_color() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        let mut d = draft("backend");
        d.role = "Cuida da API".into();
        d.args = vec!["--verbose".into()];
        let original = create_agent(&store, &t.id, &d, 1).await.unwrap();

        let copy = duplicate_agent(&store, &original.id, 2).await.unwrap();
        assert_eq!(copy.handle.as_str(), "backend-2");
        assert_eq!(copy.name, "BACKEND (cópia)");
        assert_eq!(
            (copy.role.as_str(), copy.args.clone()),
            ("Cuida da API", d.args.clone())
        );
        assert_ne!(
            copy.color, original.color,
            "a copy must be told apart on screen"
        );
        assert_ne!(copy.id, original.id);

        let third = duplicate_agent(&store, &original.id, 3).await.unwrap();
        assert_eq!(third.handle.as_str(), "backend-3");
    }

    #[tokio::test]
    async fn reorder_rewrites_positions_and_the_team_order() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        let mut ids = Vec::new();
        for handle in ["ana", "bia", "cid"] {
            ids.push(
                create_agent(&store, &t.id, &draft(handle), 1)
                    .await
                    .unwrap()
                    .id,
            );
        }
        let order = vec![ids[2].clone(), ids[0].clone(), ids[1].clone()];
        let reordered = reorder_agents(&store, &t.id, &order).await.unwrap();
        assert_eq!(
            reordered.iter().map(|a| a.position).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        let listed: Vec<_> = store
            .list_agents(&t.id)
            .await
            .unwrap()
            .into_iter()
            .map(|a| a.handle.as_str().to_owned())
            .collect();
        assert_eq!(listed, ["cid", "ana", "bia"]);
        assert!(
            reordered.iter().all(|a| a.updated_at == 1),
            "posição não é edição"
        );
    }

    #[tokio::test]
    async fn reorder_refuses_a_stale_or_partial_list() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        let a = create_agent(&store, &t.id, &draft("ana"), 1).await.unwrap();
        let b = create_agent(&store, &t.id, &draft("bia"), 1).await.unwrap();
        let bad = [
            vec![a.id.clone()],
            vec![a.id.clone(), a.id.clone()],
            vec![a.id.clone(), b.id.clone(), AgentId::new()],
            vec![b.id.clone(), AgentId::new()],
        ];
        for order in bad {
            let error = reorder_agents(&store, &t.id, &order).await.unwrap_err();
            assert_eq!(error.code(), "invalid_order");
        }
        let listed = store.list_agents(&t.id).await.unwrap();
        assert_eq!(
            (listed[0].position, listed[1].position),
            (0, 1),
            "nada gravado"
        );
    }

    #[tokio::test]
    async fn duplicate_keeps_long_handles_and_names_within_limits() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        let mut d = draft(&"a".repeat(32));
        d.name = "N".repeat(64);
        let original = create_agent(&store, &t.id, &d, 1).await.unwrap();
        let copy = duplicate_agent(&store, &original.id, 2).await.unwrap();
        assert_eq!(copy.handle.as_str(), format!("{}-2", "a".repeat(30)));
        assert_eq!(copy.name.chars().count(), 64);
        assert!(copy.name.ends_with(" (cópia)"));
    }
}
