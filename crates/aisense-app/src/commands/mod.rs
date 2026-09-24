//! Comandos expostos ao front-end.
//!
//! Camada fina de propósito (regra R6 de `AGENTS.md`): traduz UI ⇄ core e não contém
//! regra de negócio. Todo tipo que cruza esta fronteira vem do core ou do `aisense-pty`
//! e é exportado com `ts-rs` — nunca defina aqui um tipo que vire TypeScript
//! (regra R5), senão gerar tipos passa a exigir compilar a janela.

pub mod agents;
pub mod board;
pub mod bus;
pub mod notes;
pub mod notify;
pub mod project;
pub mod proposals;
pub mod pty;
pub mod push;
pub mod runtimes;
pub mod settings;
pub mod skills;
pub mod teams;
pub mod updates;

use std::sync::atomic::{AtomicBool, Ordering};

use aisense_core::{AppInfo, CommandError};
use aisense_pty::PtyError;
use tauri::{AppHandle, Manager};

/// Para tudo o que o app pôs de pé: guarda quem estava rodando (para "religar ao abrir"),
/// para os agentes, o barramento e os terminais. Roda uma vez só: depois de parar, a
/// lista de quem rodava estaria vazia e apagaria a verdadeira.
pub fn shutdown(app: &AppHandle) {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    // Antes de matar: senão a política de reinício traria os agentes de volta.
    if let Some(supervisor) = app.try_state::<agents::Supervisor>() {
        if let Some(settings) = app.try_state::<settings::Settings>() {
            settings::remember_running(&settings, &supervisor);
        }
        supervisor.shutdown();
    }
    if let Some(bus) = app.try_state::<bus::BusShutdown>() {
        bus.0.cancel();
    }
    if let Some(manager) = app.try_state::<pty::Manager>() {
        manager.shutdown();
    }
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo::current()
}

/// `PtyError` → `CommandError`.
///
/// É função e não `impl From` porque os dois tipos são de outros crates: a regra de
/// órfão do Rust não permitiria a implementação aqui.
pub fn pty_error(error: PtyError) -> CommandError {
    let code = match &error {
        PtyError::OpenPty(_) => "open_pty",
        PtyError::Spawn { .. } => "spawn_failed",
        PtyError::MissingWorkdir(_) => "missing_workdir",
        PtyError::Write(_) => "write_failed",
        PtyError::Resize(_) => "resize_failed",
        PtyError::Closed => "session_closed",
        PtyError::AlreadyRunning(_) => "already_running",
        PtyError::UnknownAgent(_) => "unknown_agent",
    };
    CommandError::new(code, error.to_string(), error.hint().map(str::to_owned))
}
