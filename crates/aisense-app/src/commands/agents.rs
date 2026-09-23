//! Comandos de ciclo de vida do agente e o evento `agent:state`.

use std::sync::Arc;

use aisense_core::agent::{
    create_agent, duplicate_agent, reorder_agents, update_agent, Agent, AgentDraft, AgentOpError,
    AgentState, AgentUpdate, Handle,
};
use aisense_core::repo::{
    AgentRepository, RepoError, SessionRecord, SessionRepository, SESSIONS_KEPT_PER_AGENT,
};
use aisense_core::state::StateConfidence;
use aisense_core::supervisor::{
    AgentPreview, AgentStateChanged, AgentSupervisor, LaunchContext, StartOutcome,
    SupervisorConfig, SupervisorError, SupervisorObserver,
};
use aisense_core::transcript::{
    export_transcript, read_transcript, summarize_sessions, SessionSummary, Transcript,
    TranscriptError,
};
use aisense_core::{now_ms, AgentId, CommandError, DataDir, SessionId, TeamId};
use aisense_pty::TerminalSize;
use aisense_store::Store;
use tauri::{AppHandle, Emitter, State};

use super::pty::{Manager, TauriSink};
use super::runtimes::Registry;

pub type Supervisor = AgentSupervisor<Store>;

pub const AGENT_STATE: &str = "agent:state";

struct TauriObserver {
    app: AppHandle,
}

impl SupervisorObserver for TauriObserver {
    fn state_changed(&self, agent_id: &AgentId, state: AgentState, confidence: StateConfidence) {
        let payload = AgentStateChanged {
            agent_id: agent_id.clone(),
            state,
            confidence,
        };
        if let Err(error) = self.app.emit(AGENT_STATE, payload) {
            tracing::warn!(agent = %agent_id, %error, "falha ao emitir o estado do agente");
        }
    }
}

pub fn setup(
    app: &AppHandle,
    data: &DataDir,
    store: Store,
    runtimes: Registry,
    pty: Manager,
) -> Supervisor {
    // Em desenvolvimento e no pacote, `aisense` e `aisense-mcp` ficam ao lado do app.
    let sidecar_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf));
    AgentSupervisor::new(
        Arc::new(store),
        runtimes,
        pty,
        Arc::new(TauriSink::new(app.clone())),
        Arc::new(TauriObserver { app: app.clone() }),
        SupervisorConfig {
            logs_dir: data.logs(),
            benches_dir: data.benches(),
            launch: LaunchContext {
                socket: data.socket(),
                sidecar_dir,
                inherited_path: std::env::var_os("PATH"),
            },
            size: TerminalSize::default(),
        },
    )
}

fn command_error(error: SupervisorError) -> CommandError {
    error.to_command_error()
}

/// Devolve onde o agente foi trabalhar (bancada ou diretório da equipe) e alguma ressalva.
#[tauri::command]
pub async fn agent_start(
    supervisor: State<'_, Supervisor>,
    agent_id: AgentId,
) -> Result<StartOutcome, CommandError> {
    supervisor.start(&agent_id).await.map_err(command_error)
}

#[tauri::command]
pub fn agent_stop(
    supervisor: State<'_, Supervisor>,
    agent_id: AgentId,
) -> Result<(), CommandError> {
    supervisor.stop(&agent_id).map_err(command_error)
}

#[tauri::command]
pub async fn agent_restart(
    supervisor: State<'_, Supervisor>,
    agent_id: AgentId,
) -> Result<StartOutcome, CommandError> {
    supervisor.restart(&agent_id).await.map_err(command_error)
}

#[tauri::command]
pub fn agent_state(supervisor: State<'_, Supervisor>, agent_id: AgentId) -> AgentState {
    supervisor.state(&agent_id)
}

fn op_error(error: AgentOpError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), None)
}

fn repo_error(error: RepoError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), None)
}

#[tauri::command]
pub async fn agents_list(
    store: State<'_, Store>,
    team_id: TeamId,
) -> Result<Vec<Agent>, CommandError> {
    store.list_agents(&team_id).await.map_err(repo_error)
}

#[tauri::command]
pub async fn agent_create(
    store: State<'_, Store>,
    team_id: TeamId,
    draft: AgentDraft,
) -> Result<Agent, CommandError> {
    create_agent(&*store, &team_id, &draft, now_ms())
        .await
        .map_err(op_error)
}

/// Editar um agente vivo grava na hora, mas só vale no próximo início; a resposta diz
/// se é preciso reiniciar (`docs/09`, F02-09).
#[tauri::command]
pub async fn agent_update(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    agent_id: AgentId,
    draft: AgentDraft,
) -> Result<AgentUpdate, CommandError> {
    let running = supervisor.state(&agent_id).is_running();
    update_agent(&*store, &agent_id, &draft, running, now_ms())
        .await
        .map_err(op_error)
}

#[tauri::command]
pub async fn agent_delete(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    agent_id: AgentId,
) -> Result<(), CommandError> {
    // Para e remove a bancada; com trabalho não commitado nela, recusa (`docs/16`).
    supervisor.retire(&agent_id).await.map_err(command_error)?;
    store.delete_agent(&agent_id).await.map_err(repo_error)
}

/// Últimas linhas da tela de cada agente, para as miniaturas da vista Foco (F03-04).
#[tauri::command]
pub fn agent_previews(
    supervisor: State<'_, Supervisor>,
    agent_ids: Vec<AgentId>,
    lines: usize,
) -> Vec<AgentPreview> {
    supervisor.previews(&agent_ids, lines)
}

/// "Duplicar" do menu do painel: mesma configuração, handle e cor livres.
#[tauri::command]
pub async fn agent_duplicate(
    store: State<'_, Store>,
    agent_id: AgentId,
) -> Result<Agent, CommandError> {
    duplicate_agent(&*store, &agent_id, now_ms())
        .await
        .map_err(op_error)
}

/// Nova ordem dos agentes da equipe, arrastada na sidebar (F03-07). Devolve a lista
/// já na ordem gravada.
#[tauri::command]
pub async fn agents_reorder(
    store: State<'_, Store>,
    team_id: TeamId,
    order: Vec<AgentId>,
) -> Result<Vec<Agent>, CommandError> {
    reorder_agents(&*store, &team_id, &order)
        .await
        .map_err(op_error)
}

/// Handle sugerido a partir do nome ("Revisão de Código" → `revisao-de-codigo`).
/// Fica no core para a regra ser uma só; o formulário chama enquanto o nome é digitado.
#[tauri::command]
pub fn handle_suggest(name: String) -> Option<String> {
    Handle::suggest(&name).map(|h| h.as_str().to_owned())
}

// ───────────── aba Logs do inspetor (F03-09) ─────────────

async fn sessions_of(
    store: &Store,
    agent_id: &AgentId,
) -> Result<Vec<SessionRecord>, CommandError> {
    store
        .list_sessions(agent_id, SESSIONS_KEPT_PER_AGENT)
        .await
        .map_err(repo_error)
}

fn transcript_error(error: TranscriptError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), None)
}

/// Ler o log é disco: fora da thread dos comandos (R6).
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, TranscriptError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| CommandError::new("internal", e.to_string(), None))?
        .map_err(transcript_error)
}

/// Sessões do agente, mais recente primeiro, com o que ainda tem transcrição.
#[tauri::command]
pub async fn agent_sessions(
    store: State<'_, Store>,
    agent_id: AgentId,
) -> Result<Vec<SessionSummary>, CommandError> {
    let sessions = sessions_of(&store, &agent_id).await?;
    blocking(move || Ok(summarize_sessions(&sessions))).await
}

/// O final da transcrição de uma sessão, já sem os códigos de terminal.
#[tauri::command]
pub async fn session_transcript(
    store: State<'_, Store>,
    agent_id: AgentId,
    session_id: SessionId,
) -> Result<Transcript, CommandError> {
    let sessions = sessions_of(&store, &agent_id).await?;
    blocking(move || read_transcript(&sessions, &session_id)).await
}

/// Grava a transcrição inteira em `path` (escolhido no diálogo de salvar).
#[tauri::command]
pub async fn session_export(
    store: State<'_, Store>,
    agent_id: AgentId,
    session_id: SessionId,
    path: String,
) -> Result<u64, CommandError> {
    let sessions = sessions_of(&store, &agent_id).await?;
    blocking(move || export_transcript(&sessions, &session_id, std::path::Path::new(&path))).await
}
