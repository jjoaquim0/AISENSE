//! Biblioteca de skills e atribuição por agente (F04-02). Evento `skills:changed` a
//! cada recarga do disco.

use std::sync::{Arc, Mutex};

use aisense_core::repo::{AgentSkill, RepoError, SkillRepository};
use aisense_core::skill::{SkillCatalog, SkillLibrary, SkillLibraryView, SkillWatcher};
use aisense_core::{now_ms, AgentId, CommandError, DataDir};
use aisense_store::Store;
use tauri::{AppHandle, Emitter, State};

pub type Library = Arc<SkillLibrary>;

/// Evento sem payload: "a biblioteca mudou, peça a lista de novo".
pub const SKILLS_CHANGED: &str = "skills:changed";

fn repo_error(error: RepoError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), None)
}

/// Carrega a biblioteca, espelha no banco e liga o hot-reload. Falhar em observar a
/// pasta não impede o app de subir — só desliga o hot-reload, com aviso no log.
pub fn setup(
    app: &AppHandle,
    data: &DataDir,
    store: &Store,
) -> (Library, Option<Mutex<SkillWatcher>>) {
    let dir = data.skills();
    let library: Library = Arc::new(SkillLibrary::new(SkillCatalog::load(&dir)));
    if let Err(error) = tauri::async_runtime::block_on(library.sync(store, now_ms())) {
        tracing::warn!(%error, "biblioteca de skills não espelhada no banco");
    }

    let for_watcher = Arc::clone(&library);
    let (handle, store) = (app.clone(), store.clone());
    let watcher = SkillWatcher::spawn(&dir, move |catalog| {
        for_watcher.set_catalog(catalog);
        let (library, store, handle) = (Arc::clone(&for_watcher), store.clone(), handle.clone());
        tauri::async_runtime::spawn(async move {
            if let Err(error) = library.sync(&store, now_ms()).await {
                tracing::warn!(%error, "skills recarregadas mas não gravadas");
            }
            if let Err(error) = handle.emit(SKILLS_CHANGED, ()) {
                tracing::warn!(%error, "falha ao avisar a UI sobre skills novas");
            }
        });
    });
    match watcher {
        Ok(watcher) => (library, Some(Mutex::new(watcher))),
        Err(error) => {
            tracing::warn!(%error, "hot-reload de skills desligado");
            (library, None)
        }
    }
}

/// A biblioteca inteira, com quem usa cada skill e os arquivos que não carregaram.
#[tauri::command]
pub async fn skills_library(
    library: State<'_, Library>,
    store: State<'_, Store>,
) -> Result<SkillLibraryView, CommandError> {
    library.view(&*store).await.map_err(repo_error)
}

/// As skills do agente, na ordem de injeção.
#[tauri::command]
pub async fn agent_skills_get(
    store: State<'_, Store>,
    agent_id: AgentId,
) -> Result<Vec<AgentSkill>, CommandError> {
    store.agent_skills(&agent_id).await.map_err(repo_error)
}

/// Troca a lista inteira; a ordem dada é a de injeção. Vale no próximo início do agente.
#[tauri::command]
pub async fn agent_skills_set(
    store: State<'_, Store>,
    agent_id: AgentId,
    skills: Vec<AgentSkill>,
) -> Result<(), CommandError> {
    store
        .set_agent_skills(&agent_id, &skills)
        .await
        .map_err(repo_error)
}
