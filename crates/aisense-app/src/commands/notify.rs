//! Notificações do SO e ícone na bandeja (F08-07).
//!
//! A decisão é do core (`aisense_core::notify::decide`); aqui só entram os fatos que só o
//! app tem — a janela está em foco? que equipe está na tela? — e as APIs do SO.

use std::sync::Mutex;

use aisense_core::agent::AgentState;
use aisense_core::notify::{decide, text, TraySummary, WindowContext};
use aisense_core::repo::{AgentRepository, TeamRepository};
use aisense_core::{now_ms, AgentId, TeamId};
use aisense_store::Store;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, State, Wry};
use tauri_plugin_notification::NotificationExt;

use super::agents::Supervisor;
use super::settings::{apply, Settings};

/// Silenciar pela bandeja vale por isto.
const MUTE_FOR_MS: i64 = 60 * 60 * 1000;

/// Equipe aberta na tela agora (informada pelo front a cada troca).
#[derive(Default)]
pub struct Viewing(Mutex<Option<TeamId>>);

#[tauri::command]
pub fn ui_viewing(viewing: State<'_, Viewing>, team_id: Option<TeamId>) {
    // Também é o marco "Sala da Equipe na tela" da auditoria de cold start (F08-01).
    tracing::info!(
        team = team_id.as_ref().map(TeamId::as_str),
        "tela da equipe"
    );
    *viewing
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = team_id;
}

/// O menu da bandeja, para trocar os textos quando o estado muda.
pub struct Tray {
    icon: TrayIcon,
    status: MenuItem<Wry>,
    mute: MenuItem<Wry>,
}

/// Cria o ícone. Um SO sem bandeja (ou Linux sem appindicator) não impede o app de subir.
pub fn setup_tray(app: &AppHandle) -> Option<Tray> {
    match build_tray(app) {
        Ok(tray) => Some(tray),
        Err(error) => {
            tracing::warn!(%error, "ícone da bandeja indisponível");
            None
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<Tray> {
    let status = MenuItem::with_id(app, "status", "Nenhum agente rodando", false, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Mostrar o AISENSE", true, None::<&str>)?;
    let mute = MenuItem::with_id(app, "mute", "Silenciar por 1 hora", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&status, &separator, &show, &mute, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("AISENSE")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_window(app),
            "mute" => toggle_mute(app),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let icon = builder.build(app)?;
    Ok(Tray { icon, status, mute })
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn toggle_mute(app: &AppHandle) {
    let Some(settings) = app.try_state::<Settings>() else {
        return;
    };
    let now = now_ms();
    let result = settings.update(|s| {
        s.notifications.muted_until = if s.notifications.muted(now) {
            None
        } else {
            Some(now + MUTE_FOR_MS)
        };
    });
    match result {
        Ok(saved) => {
            apply(app, &saved);
            refresh_tray(app);
        }
        Err(error) => tracing::warn!(message = %error.message, "silenciar pela bandeja falhou"),
    }
}

/// Atualiza a linha de resumo e o item de silenciar.
pub fn refresh_tray(app: &AppHandle) {
    let (Some(tray), Some(supervisor)) = (app.try_state::<Tray>(), app.try_state::<Supervisor>())
    else {
        return;
    };
    let line = TraySummary::of(supervisor.states()).line();
    let _ = tray.status.set_text(&line);
    let _ = tray.icon.set_tooltip(Some(format!("AISENSE — {line}")));
    let muted = app
        .try_state::<Settings>()
        .is_some_and(|s| s.get().notifications.muted(now_ms()));
    let _ = tray.mute.set_text(if muted {
        "Reativar notificações"
    } else {
        "Silenciar por 1 hora"
    });
}

/// Um agente mudou de estado: atualiza a bandeja e, se for o caso, avisa pelo SO.
pub fn state_changed(app: &AppHandle, agent_id: &AgentId, state: AgentState) {
    refresh_tray(app);
    if !matches!(state, AgentState::AwaitingInput | AgentState::Failed) {
        return;
    }
    let (app, agent_id) = (app.clone(), agent_id.clone());
    tauri::async_runtime::spawn(async move {
        let (Some(store), Some(settings)) = (app.try_state::<Store>(), app.try_state::<Settings>())
        else {
            return;
        };
        let Ok(Some(agent)) = store.get_agent(&agent_id).await else {
            return;
        };
        let team_name = match store.get_team(&agent.team_id).await {
            Ok(Some(team)) => team.name,
            _ => String::new(),
        };
        let focused = app
            .get_webview_window("main")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        let viewing = app.try_state::<Viewing>().and_then(|v| {
            v.0.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        });
        let window = WindowContext {
            focused,
            viewing: viewing.as_ref(),
        };
        let prefs = settings.get().notifications;
        let Some(kind) = decide(&prefs, now_ms(), state, &agent.team_id, window) else {
            return;
        };
        let (title, body) = text(kind, agent.handle.as_str(), &team_name);
        if let Err(error) = app.notification().builder().title(title).body(body).show() {
            tracing::warn!(%error, "notificação do sistema falhou");
        }
    });
}
