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
    // O nível de log das Configurações vale na subida; `AISENSE_LOG` ainda vence.
    let level = aisense_core::DataDir::resolve()
        .map(|d| aisense_core::settings::SettingsFile::new(d.settings()).load())
        .map_or("info", |loaded| loaded.settings.advanced.log_level.as_str());
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("AISENSE_LOG").unwrap_or_else(|_| EnvFilter::new(level)),
        )
        .init();

    tracing::info!(version = aisense_core::VERSION, "AISENSE iniciando");

    let manager: commands::pty::Manager = std::sync::Arc::new(aisense_pty::PtyManager::new());
    let shutdown_manager = std::sync::Arc::clone(&manager);
    let setup_manager = std::sync::Arc::clone(&manager);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let data = aisense_core::DataDir::resolve()
                .ok_or("could not find the user's home directory; set AISENSE_HOME")?;
            let store = open_store(&data)?;
            let settings = commands::settings::setup(&data);
            let (registry, watcher) = commands::runtimes::setup(app.handle(), &data);
            let (library, skill_watcher) = commands::skills::setup(app.handle(), &data, &store);
            let (push, injections) = commands::push::channel();
            let pty_for_push = std::sync::Arc::clone(&setup_manager);
            let supervisor = commands::agents::setup(
                app.handle(),
                &data,
                store.clone(),
                std::sync::Arc::clone(&registry),
                setup_manager,
                std::sync::Arc::clone(&library),
                push.clone(),
            );
            let secrets = std::sync::Arc::clone(&settings);
            supervisor.set_secret_env(std::sync::Arc::new(move |adapter: &str| {
                secrets.secret_env(adapter)
            }));
            let bus =
                commands::bus::setup(app.handle(), &store, &supervisor, push.clone(), &settings);
            let board = commands::board::setup(app.handle(), &bus, &store, data.benches());
            let proposals = commands::proposals::setup(app.handle(), &bus);
            let bus_shutdown = commands::bus::serve(&data, board.clone(), proposals.clone());
            commands::push::start(
                app.handle(),
                &push,
                injections,
                &bus,
                &store,
                &registry,
                &pty_for_push,
            );
            commands::settings::relaunch(&settings, &supervisor);
            app.manage(bus);
            app.manage(settings);
            app.manage(board);
            app.manage(proposals);
            app.manage(bus_shutdown);
            app.manage(store);
            app.manage(registry);
            app.manage(supervisor);
            app.manage(library);
            app.manage(commands::skills::SkillsHome(data.skills()));
            if let Some(tray) = commands::notify::setup_tray(app.handle()) {
                app.manage(tray);
            }
            if let Some(watcher) = skill_watcher {
                app.manage(watcher);
            }
            if let Some(watcher) = watcher {
                // Guardado no estado só para viver enquanto o app viver.
                app.manage(watcher);
            }
            Ok(())
        })
        .manage(manager)
        .manage(commands::notify::Viewing::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::pty::pty_spawn,
            commands::pty::pty_write,
            commands::pty::pty_resize,
            commands::pty::pty_kill,
            commands::pty::pty_snapshot,
            commands::pty::pty_clear,
            commands::pty::pty_show,
            commands::pty::pty_set_visible,
            commands::pty::pty_is_running,
            commands::runtimes::runtimes_overview,
            commands::settings::settings_get,
            commands::settings::settings_save,
            commands::settings::settings_reset,
            commands::settings::settings_onboarding_done,
            commands::settings::settings_last_team,
            commands::settings::secret_set,
            commands::settings::secret_delete,
            commands::settings::calibration_screen,
            commands::settings::calibration_test,
            commands::settings::calibration_apply,
            commands::settings::diagnostics_export,
            commands::notify::ui_viewing,
            commands::agents::agent_start,
            commands::agents::agent_stop,
            commands::agents::agent_restart,
            commands::agents::agent_state,
            commands::agents::agent_boot,
            commands::agents::agents_list,
            commands::agents::agent_create,
            commands::agents::agent_update,
            commands::agents::agent_delete,
            commands::agents::agent_duplicate,
            commands::agents::agents_reorder,
            commands::agents::agent_sessions,
            commands::agents::session_transcript,
            commands::agents::session_export,
            commands::agents::agent_previews,
            commands::agents::handle_suggest,
            commands::teams::teams_list,
            commands::teams::team_template_plan,
            commands::teams::team_create,
            commands::teams::team_set_archived,
            commands::teams::team_set_layout,
            commands::teams::team_delete,
            commands::teams::team_start,
            commands::teams::team_stop,
            commands::teams::team_restart,
            commands::skills::skills_library,
            commands::skills::agent_skills_get,
            commands::skills::agent_skills_set,
            commands::skills::agent_skills_plan,
            commands::skills::skill_check,
            commands::skills::skill_open,
            commands::skills::skill_open_file,
            commands::skills::skill_save,
            commands::skills::skill_duplicate,
            commands::skills::skill_delete,
            commands::skills::skill_import,
            commands::skills::skill_export,
            commands::skills::skill_users,
            commands::notes::notes_list,
            commands::notes::note_read,
            commands::notes::note_create,
            commands::notes::note_save,
            commands::notes::note_append,
            commands::notes::note_delete,
            commands::notes::notes_search,
            commands::bus::bus_timeline,
            commands::bus::bus_send,
            commands::bus::bus_unread,
            commands::bus::bus_resume,
            commands::bus::bus_paused,
            commands::bus::channels_list,
            commands::bus::channel_save,
            commands::bus::channel_delete,
            commands::board::board_get,
            commands::board::board_changes,
            commands::board::card_show,
            commands::board::card_add,
            commands::board::card_move,
            commands::board::card_update,
            commands::board::card_check,
            commands::board::card_comment,
            commands::board::card_link,
            commands::board::card_approve,
            commands::board::card_reject,
            commands::board::card_archive,
            commands::board::board_columns_save,
            commands::board::board_automations_save,
            commands::board::board_automations_toml,
            commands::board::board_automations_parse,
            commands::proposals::proposals_list,
            commands::proposals::proposal_decide,
            commands::project::project_lookup,
            commands::project::project_accept,
        ])
        .on_page_load(|webview, payload| {
            // Diagnóstico de subida: sem isto, uma página que não carrega no pacote é
            // uma janela em branco sem nenhuma pista no log.
            tracing::info!(
                url = %payload.url(),
                event = ?payload.event(),
                window = webview.label(),
                "página"
            );
        })
        .on_window_event(move |window, event| {
            // Fechar a janela precisa matar os processos dos agentes; senão eles
            // continuam vivos sem dono, consumindo CPU e segurando arquivos.
            if matches!(event, tauri::WindowEvent::Destroyed) {
                // Antes de matar: senão a política de reinício traria os agentes de volta.
                if let Some(supervisor) = window.try_state::<commands::agents::Supervisor>() {
                    if let Some(settings) = window.try_state::<commands::settings::Settings>() {
                        commands::settings::remember_running(&settings, &supervisor);
                    }
                    supervisor.shutdown();
                }
                if let Some(bus) = window.try_state::<commands::bus::BusShutdown>() {
                    bus.0.cancel();
                }
                shutdown_manager.shutdown();
            }
        })
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a janela do AISENSE");
}
