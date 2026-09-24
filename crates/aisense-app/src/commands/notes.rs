//! Notas da equipe (F04-09, `docs/15`): `<workdir>/.aisense/notes/`. A CLI dos agentes
//! (`aisense notes ...`) chega com o servidor de IPC (Fase 05) e usa o mesmo core.

use std::path::Path;

use aisense_core::notes::{Note, NoteError, NoteMatch, NoteSave, NoteSummary, TeamNotes};
use aisense_core::repo::{RepoError, TeamRepository};
use aisense_core::{CommandError, TeamId};
use aisense_store::Store;
use tauri::State;

async fn team_notes(store: &Store, team_id: &TeamId) -> Result<TeamNotes, CommandError> {
    let team = store
        .get_team(team_id)
        .await
        .map_err(|e| CommandError::new(e.code(), e.to_string(), None))?
        .ok_or_else(|| {
            let error = RepoError::TeamNotFound(team_id.clone());
            CommandError::new(error.code(), error.to_string(), None)
        })?;
    Ok(TeamNotes::new(Path::new(&team.workdir)))
}

fn note_error(error: NoteError) -> CommandError {
    error.to_command_error()
}

/// Mais recente primeiro.
#[tauri::command]
pub async fn notes_list(
    store: State<'_, Store>,
    team_id: TeamId,
) -> Result<Vec<NoteSummary>, CommandError> {
    team_notes(&store, &team_id)
        .await?
        .list()
        .map_err(note_error)
}

#[tauri::command]
pub async fn note_read(
    store: State<'_, Store>,
    team_id: TeamId,
    slug: String,
) -> Result<Note, CommandError> {
    team_notes(&store, &team_id)
        .await?
        .read(&slug)
        .map_err(note_error)
}

#[tauri::command]
pub async fn note_create(
    store: State<'_, Store>,
    team_id: TeamId,
    slug: String,
    title: String,
) -> Result<Note, CommandError> {
    team_notes(&store, &team_id)
        .await?
        .create(&slug, &title)
        .map_err(note_error)
}

/// Substitui com trava otimista: nota mudada desde a leitura volta como `stale`, com o diff.
#[tauri::command]
pub async fn note_save(
    store: State<'_, Store>,
    team_id: TeamId,
    slug: String,
    content: String,
    expect_hash: Option<String>,
) -> Result<NoteSave, CommandError> {
    team_notes(&store, &team_id)
        .await?
        .save(&slug, &content, expect_hash.as_deref())
        .map_err(note_error)
}

#[tauri::command]
pub async fn note_append(
    store: State<'_, Store>,
    team_id: TeamId,
    slug: String,
    text: String,
) -> Result<Note, CommandError> {
    team_notes(&store, &team_id)
        .await?
        .append(&slug, &text)
        .map_err(note_error)
}

#[tauri::command]
pub async fn note_delete(
    store: State<'_, Store>,
    team_id: TeamId,
    slug: String,
) -> Result<(), CommandError> {
    team_notes(&store, &team_id)
        .await?
        .delete(&slug)
        .map_err(note_error)
}

#[tauri::command]
pub async fn notes_search(
    store: State<'_, Store>,
    team_id: TeamId,
    query: String,
) -> Result<Vec<NoteMatch>, CommandError> {
    team_notes(&store, &team_id)
        .await?
        .search(&query)
        .map_err(note_error)
}
