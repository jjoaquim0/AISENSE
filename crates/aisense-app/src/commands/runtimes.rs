//! Comandos e eventos de runtimes (adaptadores + detecção).

use std::sync::{Arc, Mutex};

use aisense_core::adapter::{AdapterCatalog, AdapterWatcher, RuntimeOverview, RuntimeRegistry};
use aisense_core::{CommandError, DataDir};
use tauri::{AppHandle, Emitter, Manager, State};

pub type Registry = Arc<RuntimeRegistry>;

/// Evento sem payload: "o catálogo mudou, peça a lista de novo".
pub const ADAPTERS_CHANGED: &str = "adapters:changed";

/// Carrega o catálogo e liga o hot-reload. Falhar em observar a pasta não impede o
/// app de subir — só desliga o hot-reload, com aviso no log.
pub fn setup(app: &AppHandle, data: &DataDir) -> (Registry, Option<Mutex<AdapterWatcher>>) {
    let dir = data.adapters();
    let registry: Registry = Arc::new(RuntimeRegistry::new(AdapterCatalog::load(&dir)));

    let for_watcher = Arc::clone(&registry);
    let handle = app.clone();
    let watcher = AdapterWatcher::spawn(&dir, move |catalog| {
        for_watcher.set_catalog(catalog);
        // Regras de estado novas valem já nas sessões vivas (modo calibração, F08-05).
        if let Some(supervisor) = handle.try_state::<super::agents::Supervisor>() {
            supervisor.refresh_state_rules();
        }
        if let Err(error) = handle.emit(ADAPTERS_CHANGED, ()) {
            tracing::warn!(%error, "falha ao avisar a UI sobre adaptadores novos");
        }
    });
    match watcher {
        Ok(watcher) => (registry, Some(Mutex::new(watcher))),
        Err(error) => {
            tracing::warn!(%error, "hot-reload de adaptadores desligado");
            (registry, None)
        }
    }
}

/// Lista os runtimes com o estado de cada um. A detecção roda fora da thread da UI
/// (regra R6): pode levar até o timeout de 3 s.
#[tauri::command]
pub async fn runtimes_overview(
    registry: State<'_, Registry>,
    refresh: bool,
) -> Result<RuntimeOverview, CommandError> {
    let registry = Arc::clone(&registry);
    tauri::async_runtime::spawn_blocking(move || registry.overview(refresh))
        .await
        .map_err(|error| {
            CommandError::new(
                "detection_failed",
                format!("runtime detection stopped unexpectedly: {error}"),
                None,
            )
        })
}
