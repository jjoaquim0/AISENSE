//! Aplicativo Tauri do AISENSE.
//!
//! Esta camada é **fina de propósito**: ela traduz UI ⇄ core e não contém regra de
//! negócio. Ver `docs/02-arquitetura.md` e a regra R6 de `AGENTS.md`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod commands;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("AISENSE_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!(version = aisense_core::VERSION, "AISENSE iniciando");

    let manager: commands::pty::Manager = std::sync::Arc::new(aisense_pty::PtyManager::new());
    let shutdown_manager = std::sync::Arc::clone(&manager);

    tauri::Builder::default()
        .manage(manager)
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::pty::pty_spawn,
            commands::pty::pty_write,
            commands::pty::pty_resize,
            commands::pty::pty_kill,
            commands::pty::pty_snapshot,
            commands::pty::pty_set_visible,
            commands::pty::pty_is_running,
        ])
        .on_window_event(move |_window, event| {
            // Fechar a janela precisa matar os processos dos agentes; senão eles
            // continuam vivos sem dono, consumindo CPU e segurando arquivos.
            if matches!(event, tauri::WindowEvent::Destroyed) {
                shutdown_manager.shutdown();
            }
        })
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a janela do AISENSE");
}
