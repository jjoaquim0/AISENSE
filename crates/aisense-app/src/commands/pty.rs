//! Comandos e eventos de terminal.

use std::sync::Arc;

use aisense_core::CommandError;
use aisense_pty::{OutputSink, PtyData, PtyExit, PtyManager, PtySpawn, SpawnRequest, TerminalSize};
use base64::Engine;
use tauri::{AppHandle, Emitter, State};

use super::pty_error;

/// Destino que transforma a saída coalescida em eventos Tauri.
pub struct TauriSink {
    app: AppHandle,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl OutputSink for TauriSink {
    fn data(&self, agent_id: &str, chunk: Vec<u8>) {
        let payload = PtyData {
            agent_id: agent_id.to_owned(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(&chunk),
        };
        if let Err(error) = self.app.emit("pty:data", payload) {
            tracing::warn!(%agent_id, %error, "falha ao emitir saída do terminal");
        }
    }

    fn exit(&self, agent_id: &str, code: i32) {
        let payload = PtyExit {
            agent_id: agent_id.to_owned(),
            code,
        };
        if let Err(error) = self.app.emit("pty:exit", payload) {
            tracing::warn!(%agent_id, %error, "falha ao emitir término do terminal");
        }
    }
}

pub type Manager = Arc<PtyManager>;

#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    manager: State<'_, Manager>,
    request: SpawnRequest,
) -> Result<(), CommandError> {
    let agent_id = request.agent_id.clone();
    let spec: PtySpawn = request.into();
    let sink = Arc::new(TauriSink { app });
    manager.spawn(agent_id, spec, sink).map_err(pty_error)
}

#[tauri::command]
pub fn pty_write(
    manager: State<'_, Manager>,
    agent_id: String,
    data: String,
) -> Result<(), CommandError> {
    manager.write(&agent_id, data.as_bytes()).map_err(pty_error)
}

#[tauri::command]
pub fn pty_resize(
    manager: State<'_, Manager>,
    supervisor: State<'_, super::agents::Supervisor>,
    agent_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), CommandError> {
    let size = TerminalSize { rows, cols };
    manager.resize(&agent_id, size).map_err(pty_error)?;
    // O detector de estado lê a mesma tela que o painel mostra.
    supervisor.resized(&aisense_core::AgentId::from_raw(agent_id), size);
    Ok(())
}

#[tauri::command]
pub fn pty_kill(manager: State<'_, Manager>, agent_id: String) -> Result<(), CommandError> {
    manager.kill(&agent_id).map_err(pty_error)
}

/// Histórico retido, em base64, para reidratar o xterm de uma vez só.
#[tauri::command]
pub fn pty_snapshot(manager: State<'_, Manager>, agent_id: String) -> Result<String, CommandError> {
    let bytes = manager.snapshot(&agent_id).map_err(pty_error)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// "Limpar" do menu do painel: esquece o histórico retido (o log em disco fica).
#[tauri::command]
pub fn pty_clear(manager: State<'_, Manager>, agent_id: String) -> Result<(), CommandError> {
    manager.clear(&agent_id).map_err(pty_error)
}

#[tauri::command]
pub fn pty_set_visible(
    manager: State<'_, Manager>,
    agent_id: String,
    visible: bool,
) -> Result<(), CommandError> {
    manager.set_visible(&agent_id, visible).map_err(pty_error)
}

#[tauri::command]
pub fn pty_is_running(manager: State<'_, Manager>, agent_id: String) -> bool {
    manager.is_running(&agent_id)
}
