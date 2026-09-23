//! Comandos de ciclo de vida do agente e o evento `agent:state`.

use std::sync::Arc;

use aisense_core::agent::AgentState;
use aisense_core::supervisor::{
    AgentStateChanged, AgentSupervisor, LaunchContext, SupervisorConfig, SupervisorError,
    SupervisorObserver,
};
use aisense_core::{AgentId, CommandError, DataDir};
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
    fn state_changed(&self, agent_id: &AgentId, state: AgentState) {
        let payload = AgentStateChanged {
            agent_id: agent_id.clone(),
            state,
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

#[tauri::command]
pub async fn agent_start(
    supervisor: State<'_, Supervisor>,
    agent_id: AgentId,
) -> Result<(), CommandError> {
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
) -> Result<(), CommandError> {
    supervisor.restart(&agent_id).await.map_err(command_error)
}

#[tauri::command]
pub fn agent_state(supervisor: State<'_, Supervisor>, agent_id: AgentId) -> AgentState {
    supervisor.state(&agent_id)
}
