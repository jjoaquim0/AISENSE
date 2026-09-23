//! Comandos de equipe (`docs/09`, T2 e T3).

use std::sync::Arc;

use aisense_core::adapter::RuntimeStatus;
use aisense_core::agent::AgentDraft;
use aisense_core::repo::{AgentRepository, RepoError, TeamFilter, TeamRepository};
use aisense_core::supervisor::AgentStartFailure;
use aisense_core::team::{
    confirm_deletion, create_team_with_agents, AgentSummary, PlannedAgent, Team, TeamDraft,
    TeamSetupError, TeamSummary, TeamTemplate,
};
use aisense_core::{now_ms, CommandError, TeamId};
use aisense_store::Store;
use tauri::State;

use super::agents::Supervisor;
use super::runtimes::Registry;

fn repo_error(error: RepoError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), None)
}

fn setup_error(error: TeamSetupError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), None)
}

async fn load_team(store: &Store, team_id: &TeamId) -> Result<Team, CommandError> {
    store
        .get_team(team_id)
        .await
        .map_err(repo_error)?
        .ok_or_else(|| repo_error(RepoError::TeamNotFound(team_id.clone())))
}

/// Para todos os agentes da equipe. Usado antes de arquivar e de excluir: um processo
/// vivo de uma equipe que sumiu da tela ficaria órfão.
async fn stop_team(
    store: &Store,
    supervisor: &Supervisor,
    team_id: &TeamId,
) -> Result<(), CommandError> {
    for agent in store.list_agents(team_id).await.map_err(repo_error)? {
        if let Err(error) = supervisor.stop(&agent.id) {
            tracing::warn!(agent = %agent.id, %error, "agente não parou");
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn teams_list(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    include_archived: bool,
) -> Result<Vec<TeamSummary>, CommandError> {
    let teams = store
        .list_teams(TeamFilter { include_archived })
        .await
        .map_err(repo_error)?;
    let mut summaries = Vec::with_capacity(teams.len());
    for team in teams {
        let agents = store
            .list_agents(&team.id)
            .await
            .map_err(repo_error)?
            .iter()
            .map(|agent| AgentSummary::new(agent, supervisor.state(&agent.id)))
            .collect();
        summaries.push(TeamSummary { team, agents });
    }
    Ok(summaries)
}

/// Os agentes de um modelo, com o runtime já trocado quando o preferido não está
/// instalado (T3, passo 3). Pode esperar a detecção de runtimes (até 3 s).
#[tauri::command]
pub async fn team_template_plan(
    registry: State<'_, Registry>,
    template: TeamTemplate,
) -> Result<Vec<PlannedAgent>, CommandError> {
    let registry = Arc::clone(&registry);
    tauri::async_runtime::spawn_blocking(move || {
        let overview = registry.overview(false);
        template.plan(|id| {
            overview
                .runtimes
                .iter()
                .any(|r| r.adapter.id == id && matches!(r.status, RuntimeStatus::Available { .. }))
        })
    })
    .await
    .map_err(|error| CommandError::new("detection_failed", error.to_string(), None))
}

#[tauri::command]
pub async fn team_create(
    store: State<'_, Store>,
    team: TeamDraft,
    agents: Vec<AgentDraft>,
) -> Result<Team, CommandError> {
    let (team, _) = create_team_with_agents(&*store, &team, &agents, now_ms())
        .await
        .map_err(setup_error)?;
    tracing::info!(team = %team.id, agents = agents.len(), "equipe criada");
    Ok(team)
}

#[tauri::command]
pub async fn team_set_archived(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    team_id: TeamId,
    archived: bool,
) -> Result<(), CommandError> {
    if archived {
        stop_team(&store, &supervisor, &team_id).await?;
    }
    store
        .set_team_archived(&team_id, archived.then(now_ms))
        .await
        .map_err(repo_error)
}

/// Excluir apaga tudo o que é da equipe; o nome digitado é conferido aqui também,
/// não só na interface.
#[tauri::command]
pub async fn team_delete(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    team_id: TeamId,
    confirm_name: String,
) -> Result<(), CommandError> {
    let team = load_team(&store, &team_id).await?;
    confirm_deletion(&team, &confirm_name).map_err(setup_error)?;
    stop_team(&store, &supervisor, &team_id).await?;
    store.delete_team(&team_id).await.map_err(repo_error)?;
    tracing::info!(team = %team_id, "equipe excluída");
    Ok(())
}

/// "▶ Iniciar equipe": sobe todos os agentes com `autostart` que estão parados.
/// Devolve os que não subiram, cada um com o motivo.
#[tauri::command]
pub async fn team_start(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    team_id: TeamId,
) -> Result<Vec<AgentStartFailure>, CommandError> {
    let mut failures = Vec::new();
    for agent in store.list_agents(&team_id).await.map_err(repo_error)? {
        if !agent.autostart || supervisor.state(&agent.id).is_running() {
            continue;
        }
        if let Err(error) = supervisor.start(&agent.id).await {
            failures.push(AgentStartFailure {
                agent_id: agent.id.clone(),
                error: error.to_command_error(),
            });
        }
    }
    Ok(failures)
}
