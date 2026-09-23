//! Aplicativo Tauri do AISENSE.
//!
//! Esta camada é **fina de propósito**: ela traduz UI ⇄ core e não contém regra de
//! negócio. Ver `docs/02-arquitetura.md` e a regra R6 de `AGENTS.md`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod commands;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

/// Abre o banco e aplica as migrações antes da janela existir: se falhar, o app
/// não deve subir pela metade, com a UI mostrando dados que não persistem.
fn open_store(
    data: &aisense_core::DataDir,
) -> Result<aisense_store::Store, Box<dyn std::error::Error>> {
    let path = data.database();
    Ok(tauri::async_runtime::block_on(aisense_store::Store::open(
        &path,
    ))?)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("AISENSE_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!(version = aisense_core::VERSION, "AISENSE iniciando");

    let manager: commands::pty::Manager = std::sync::Arc::new(aisense_pty::PtyManager::new());
    let shutdown_manager = std::sync::Arc::clone(&manager);
    let setup_manager = std::sync::Arc::clone(&manager);

    tauri::Builder::default()
        .setup(move |app| {
            let data = aisense_core::DataDir::resolve()
                .ok_or("could not find the user's home directory; set AISENSE_HOME")?;
            let store = open_store(&data)?;
            let (registry, watcher) = commands::runtimes::setup(app.handle(), &data);
            let supervisor = commands::agents::setup(
                app.handle(),
                &data,
                store.clone(),
                std::sync::Arc::clone(&registry),
                setup_manager,
            );
            app.manage(store);
            app.manage(registry);
            app.manage(supervisor);
            if let Some(watcher) = watcher {
                // Guardado no estado só para viver enquanto o app viver.
                app.manage(watcher);
            }
            Ok(())
        })
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
            commands::runtimes::runtimes_overview,
            commands::agents::agent_start,
            commands::agents::agent_stop,
            commands::agents::agent_restart,
            commands::agents::agent_state,
        ])
        .on_window_event(move |window, event| {
            // Fechar a janela precisa matar os processos dos agentes; senão eles
            // continuam vivos sem dono, consumindo CPU e segurando arquivos.
            if matches!(event, tauri::WindowEvent::Destroyed) {
                // Antes de matar: senão a política de reinício traria os agentes de volta.
                if let Some(supervisor) = window.try_state::<commands::agents::Supervisor>() {
                    supervisor.shutdown();
                }
                shutdown_manager.shutdown();
            }
        })
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a janela do AISENSE");
}
