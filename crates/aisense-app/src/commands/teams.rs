//! Comandos de equipe (`docs/09`, T2 e T3).

use std::sync::Arc;

use aisense_core::adapter::RuntimeStatus;
use aisense_core::agent::AgentDraft;
use aisense_core::bench;
use aisense_core::repo::{AgentRepository, RepoError, TeamFilter, TeamRepository};
use aisense_core::supervisor::{
    SupervisorError, TeamProgress, TeamStartReport, TEAM_START_STAGGER,
};
use aisense_core::team::{
    confirm_deletion, create_team_with_agents, AgentSummary, PlannedAgent, Team, TeamDraft,
    TeamSetupError, TeamSummary, TeamTemplate,
};
use aisense_core::{now_ms, CommandError, TeamId};
use aisense_store::Store;
use tauri::{AppHandle, Emitter, State};

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

/// Para todos os agentes da equipe e espera saírem. Usado antes de arquivar e de
/// excluir: um processo vivo de uma equipe que sumiu da tela ficaria órfão.
async fn stop_team(supervisor: &Supervisor, team_id: &TeamId) -> Result<(), CommandError> {
    supervisor
        .stop_team(team_id, &|_| {})
        .await
        .map_err(|e| e.to_command_error())
}

fn command_error(error: SupervisorError) -> CommandError {
    error.to_command_error()
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

/// Modo de vista e posição dos painéis da Sala da Equipe (F03-03).
#[tauri::command]
pub async fn team_set_layout(
    store: State<'_, Store>,
    team_id: TeamId,
    layout: serde_json::Value,
) -> Result<(), CommandError> {
    aisense_core::team::save_layout(&*store, &team_id, layout)
        .await
        .map_err(|e| CommandError::new(e.code(), e.to_string(), None))
}

#[tauri::command]
pub async fn team_set_archived(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    team_id: TeamId,
    archived: bool,
) -> Result<(), CommandError> {
    if archived {
        stop_team(&supervisor, &team_id).await?;
    }
    store
        .set_team_archived(&team_id, archived.then(now_ms))
        .await
        .map_err(repo_error)
}

/// Excluir apaga tudo o que é da equipe; o nome digitado é conferido aqui também,
/// não só na interface. Bancadas com trabalho não commitado impedem a exclusão, com a
/// lista de quem tem pendência (`docs/16`, "Ciclo de vida e limpeza").
#[tauri::command]
pub async fn team_delete(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    team_id: TeamId,
    confirm_name: String,
) -> Result<(), CommandError> {
    let team = load_team(&store, &team_id).await?;
    confirm_deletion(&team, &confirm_name).map_err(setup_error)?;
    let agents = store.list_agents(&team_id).await.map_err(repo_error)?;

    let benches = supervisor.benches_dir().to_path_buf();
    let (t, list) = (team.clone(), agents.clone());
    let pending = tauri::async_runtime::spawn_blocking(move || {
        list.iter()
            .filter_map(|agent| match bench::pending_changes(&benches, &t, agent) {
                Ok(changes) if !changes.is_empty() => {
                    Some(format!("@{} ({})", agent.handle.as_str(), changes.len()))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| CommandError::new("bench_io", e.to_string(), None))?;
    if !pending.is_empty() {
        return Err(CommandError::new(
            "bench_dirty",
            format!("benches with uncommitted changes: {}", pending.join(", ")),
            Some(format!(
                "Commite ou descarte as mudanças nas bancadas de {} antes de excluir a equipe.",
                pending.join(", ")
            )),
        ));
    }

    for agent in &agents {
        supervisor
            .retire(&agent.id)
            .await
            .map_err(|e| e.to_command_error())?;
    }
    store.delete_team(&team_id).await.map_err(repo_error)?;
    tracing::info!(team = %team_id, "equipe excluída");
    Ok(())
}

/// "▶ Iniciar equipe": sobe todos os agentes com `autostart` que estão parados.
/// Devolve quem não subiu (com o motivo) e quem subiu com ressalva (sem bancada...).
pub const TEAM_PROGRESS: &str = "team:progress";

/// Emite o avanço de uma operação da equipe para a interface (`team:progress`).
fn emitter(app: &AppHandle) -> impl Fn(TeamProgress) + Send + Sync + '_ {
    move |progress| {
        if let Err(error) = app.emit(TEAM_PROGRESS, progress) {
            tracing::warn!(%error, "falha ao emitir o progresso da equipe");
        }
    }
}

/// ▶ Iniciar equipe: os `autostart`, na ordem, com 300 ms entre um e outro (F03-06).
#[tauri::command]
pub async fn team_start(
    app: AppHandle,
    supervisor: State<'_, Supervisor>,
    team_id: TeamId,
) -> Result<TeamStartReport, CommandError> {
    supervisor
        .start_team(&team_id, TEAM_START_STAGGER, &emitter(&app))
        .await
        .map_err(command_error)
}

/// ⏸ Parar tudo.
#[tauri::command]
pub async fn team_stop(
    app: AppHandle,
    supervisor: State<'_, Supervisor>,
    team_id: TeamId,
) -> Result<(), CommandError> {
    supervisor
        .stop_team(&team_id, &emitter(&app))
        .await
        .map_err(command_error)
}

/// ⟳ Reiniciar tudo: quem estava rodando volta, escalonado.
#[tauri::command]
pub async fn team_restart(
    app: AppHandle,
    supervisor: State<'_, Supervisor>,
    team_id: TeamId,
) -> Result<TeamStartReport, CommandError> {
    supervisor
        .restart_team(&team_id, TEAM_START_STAGGER, &emitter(&app))
        .await
        .map_err(command_error)
}
