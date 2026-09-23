//! Biblioteca de skills e atribuição por agente (F04-02). Evento `skills:changed` a
//! cada recarga do disco.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use aisense_core::repo::{AgentRepository, AgentSkill, RepoError, SkillRepository};
use aisense_core::skill::{
    self as skill, resolve_agent_skills, OpenedSkill, Skill, SkillCatalog, SkillCheck,
    SkillEditError, SkillLibrary, SkillLibraryView, SkillPlan, SkillUser, SkillWatcher,
};
use aisense_core::{now_ms, AgentId, CommandError, DataDir};
use aisense_store::Store;
use tauri::{AppHandle, Emitter, State};

use super::agents::Supervisor;
use super::runtimes::Registry;

pub type Library = Arc<SkillLibrary>;

/// `~/.aisense/skills/`: a única pasta em que o editor escreve.
pub struct SkillsHome(pub PathBuf);

/// Evento sem payload: "a biblioteca mudou, peça a lista de novo".
pub const SKILLS_CHANGED: &str = "skills:changed";

fn repo_error(error: RepoError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), None)
}

fn edit_error(error: SkillEditError) -> CommandError {
    error.to_command_error()
}

/// Um `adapter_id` que o catálogo de runtimes conhece (embutido ou do usuário).
fn known_runtimes(registry: &Registry) -> impl Fn(&str) -> bool {
    let catalog = registry.catalog();
    move |id| catalog.get(id).is_some()
}

/// Depois de escrever na pasta: recarrega já, sem esperar o debounce do observador, para
/// a biblioteca da UI e o banco refletirem a mudança na volta do comando.
async fn reload(app: &AppHandle, library: &SkillLibrary, store: &Store, dir: &Path) {
    library.set_catalog(SkillCatalog::load(dir));
    if let Err(error) = library.sync(store, now_ms()).await {
        tracing::warn!(%error, "skills recarregadas mas não gravadas");
    }
    if let Err(error) = app.emit(SKILLS_CHANGED, ()) {
        tracing::warn!(%error, "falha ao avisar a UI sobre skills novas");
    }
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

/// O que o agente levaria se subisse agora: as skills ativas, em ordem, e as ignoradas
/// com o porquê (runtime incompatível, fora do disco). F04-03.
#[tauri::command]
pub async fn agent_skills_plan(
    library: State<'_, Library>,
    store: State<'_, Store>,
    agent_id: AgentId,
) -> Result<SkillPlan, CommandError> {
    let agent = store
        .get_agent(&agent_id)
        .await
        .map_err(repo_error)?
        .ok_or_else(|| repo_error(RepoError::AgentNotFound(agent_id.clone())))?;
    let resolved = resolve_agent_skills(&*store, &library.catalog(), &agent_id, &agent.adapter_id)
        .await
        .map_err(repo_error)?;
    Ok(resolved.plan())
}

/// Valida o rascunho do editor a cada tecla (F04-08). Não escreve nada.
#[tauri::command]
pub async fn skill_check(
    library: State<'_, Library>,
    registry: State<'_, Registry>,
    source: String,
    editing: Option<String>,
) -> Result<SkillCheck, CommandError> {
    Ok(skill::check_skill(
        &source,
        &library.catalog(),
        known_runtimes(&registry),
        editing.as_deref(),
    ))
}

/// O `SKILL.md` de uma skill da biblioteca, para o editor.
#[tauri::command]
pub async fn skill_open(
    library: State<'_, Library>,
    name: String,
) -> Result<OpenedSkill, CommandError> {
    skill::open_skill(&library.catalog(), &name).map_err(edit_error)
}

/// Um `SKILL.md` que não carregou, aberto pelo caminho para ser consertado.
#[tauri::command]
pub async fn skill_open_file(
    home: State<'_, SkillsHome>,
    path: String,
) -> Result<OpenedSkill, CommandError> {
    skill::open_skill_file(&home.0, Path::new(&path)).map_err(edit_error)
}

/// Salva: nova (`editing` vazio) cria a pasta; edição reescreve a pasta aberta.
#[tauri::command]
pub async fn skill_save(
    app: AppHandle,
    home: State<'_, SkillsHome>,
    library: State<'_, Library>,
    registry: State<'_, Registry>,
    store: State<'_, Store>,
    source: String,
    editing: Option<String>,
) -> Result<Skill, CommandError> {
    let saved = skill::save_skill(
        &home.0,
        &library.catalog(),
        known_runtimes(&registry),
        &source,
        editing.as_deref(),
    )
    .map_err(edit_error)?;
    reload(&app, &library, &store, &home.0).await;
    Ok(saved)
}

/// O rascunho de uma cópia, com nome livre. Não salva — o editor abre como skill nova.
#[tauri::command]
pub async fn skill_duplicate(
    home: State<'_, SkillsHome>,
    library: State<'_, Library>,
    name: String,
) -> Result<String, CommandError> {
    skill::duplicate_source(&home.0, &library.catalog(), &name).map_err(edit_error)
}

/// Apaga a pasta de uma skill do usuário; as atribuições ficam como "fora do disco".
#[tauri::command]
pub async fn skill_delete(
    app: AppHandle,
    home: State<'_, SkillsHome>,
    library: State<'_, Library>,
    store: State<'_, Store>,
    dir: String,
) -> Result<(), CommandError> {
    skill::delete_skill(&home.0, &dir).map_err(edit_error)?;
    reload(&app, &library, &store, &home.0).await;
    Ok(())
}

/// Copia uma pasta de skill (ou a de um `SKILL.md`) para a biblioteca.
#[tauri::command]
pub async fn skill_import(
    app: AppHandle,
    home: State<'_, SkillsHome>,
    library: State<'_, Library>,
    registry: State<'_, Registry>,
    store: State<'_, Store>,
    path: String,
) -> Result<Skill, CommandError> {
    let imported = skill::import_skill(
        &home.0,
        &library.catalog(),
        known_runtimes(&registry),
        Path::new(&path),
    )
    .map_err(edit_error)?;
    reload(&app, &library, &store, &home.0).await;
    Ok(imported)
}

/// Escreve a skill em `<to>/<nome>/`. Devolve a pasta criada.
#[tauri::command]
pub async fn skill_export(
    library: State<'_, Library>,
    name: String,
    to: String,
) -> Result<String, CommandError> {
    skill::export_skill(&library.catalog(), &name, Path::new(&to))
        .map(|dir| dir.display().to_string())
        .map_err(edit_error)
}

/// Quem usa a skill, com equipe e estado: a lista exata do "precisam reiniciar".
#[tauri::command]
pub async fn skill_users(
    store: State<'_, Store>,
    supervisor: State<'_, Supervisor>,
    name: String,
) -> Result<Vec<SkillUser>, CommandError> {
    skill::skill_users(&*store, &name, |id| supervisor.state(id))
        .await
        .map_err(repo_error)
}
