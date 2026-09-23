//! Comandos expostos ao front-end.
//!
//! Camada fina de propósito (regra R6 de `AGENTS.md`): traduz UI ⇄ core e não contém
//! regra de negócio. Todo tipo que cruza esta fronteira vem do core ou do `aisense-pty`
//! e é exportado com `ts-rs` — nunca defina aqui um tipo que vire TypeScript
//! (regra R5), senão gerar tipos passa a exigir compilar a janela.

pub mod agents;
pub mod project;
pub mod pty;
pub mod runtimes;
pub mod teams;

use aisense_core::{AppInfo, CommandError};
use aisense_pty::PtyError;

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
