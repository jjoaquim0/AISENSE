//! Auto-update assinado (F09-03): procura no canal estável, baixa, confere a assinatura
//! contra a chave pública da config do app, para os agentes e reinicia. O canal estável é
//! o `releases/latest/download/latest.json` de `plugins.updater.endpoints`: o `/latest`
//! do GitHub ignora pre-releases (tags com hífen).

use std::sync::{Mutex, PoisonError};

use aisense_core::updates::{UpdateInfo, UpdateProgress};
use aisense_core::CommandError;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, Updater, UpdaterExt};

/// Payload: [`UpdateProgress`].
pub const UPDATE_PROGRESS: &str = "update:progress";

/// A versão achada no último `update_check`, para o `update_install` instalar exatamente ela.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

fn failed(action: &str, error: impl std::fmt::Display) -> CommandError {
    CommandError::new(
        "update_failed",
        format!("could not {action}: {error}"),
        Some("Confira a conexão e tente de novo; se continuar, baixe a versão nova em github.com/jjoaquim0/AISENSE/releases.".into()),
    )
}

/// A chave pública de `plugins.updater.pubkey`, que o workflow de release grava na
/// config a partir do segredo `AISENSE_UPDATER_PUBKEY` (`docs/distribuicao.md`). O
/// `tauri.conf.json` versionado a deixa vazia: build de desenvolvimento não procura.
fn configured_pubkey(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

/// Endpoints e chave vêm de `plugins.updater` no `tauri.conf.json`.
fn updater(app: &AppHandle) -> Result<Updater, CommandError> {
    if !configured_pubkey(app) {
        return Err(CommandError::new(
            "updates_disabled",
            "this build has no updater public key",
            Some("Esta compilação não procura atualizações (build de desenvolvimento).".into()),
        ));
    }
    app.updater()
        .map_err(|e| failed("configure the updater", e))
}

/// `None` quando já está na versão mais nova.
#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>, CommandError> {
    let update = updater(&app)?
        .check()
        .await
        .map_err(|e| failed("check for updates", e))?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
        date: u.date.map(|d| d.to_string()),
    });
    if let Some(info) = &info {
        tracing::info!(version = %info.version, "atualização disponível");
    }
    *pending.0.lock().unwrap_or_else(PoisonError::into_inner) = update;
    Ok(info)
}

/// Baixa com progresso (`update:progress`), confere a assinatura, para os agentes,
/// instala e reinicia. Só volta se algo falhar antes de reiniciar — e aí nada mudou.
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<(), CommandError> {
    let update = pending
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .ok_or_else(|| {
            CommandError::new(
                "no_update",
                "there is no update to install",
                Some("Procure atualizações de novo.".into()),
            )
        })?;
    let mut downloaded = 0u64;
    let progress = app.clone();
    // `download` confere a assinatura: um pacote adulterado para aqui, sem instalar.
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit(UPDATE_PROGRESS, UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| failed("download the update", e))?;
    tracing::info!(version = %update.version, "atualização baixada e verificada; parando os agentes");
    // Antes de instalar: no Windows o instalador fecha o app, e os agentes não podem
    // ficar órfãos nem perder o "religar ao abrir".
    super::shutdown(&app);
    update
        .install(bytes)
        .map_err(|e| failed("install the update", e))?;
    app.restart();
}
