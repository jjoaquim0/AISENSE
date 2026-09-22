//! `TeamRepository` sobre SQLite.

use aisense_core::agent::WorkspaceMode;
use aisense_core::repo::{RepoError, RepoResult, TeamFilter, TeamRepository};
use aisense_core::team::Team;
use aisense_core::{AgentColor, Millis, TeamId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::convert::{backend, is_primary_key_conflict, json, text_enum, to_json};
use crate::Store;

const COLUMNS: &str = "id, name, mission, workdir, color, icon, workspace_mode, layout, \
                       archived_at, created_at, updated_at";

fn from_row(row: &SqliteRow) -> RepoResult<Team> {
    let get = |col: &str| row.try_get::<String, _>(col).map_err(backend);
    Ok(Team {
        id: TeamId::from_raw(get("id")?),
        name: get("name")?,
        mission: get("mission")?,
        workdir: get("workdir")?,
        color: text_enum("teams", "color", &get("color")?, AgentColor::parse)?,
        icon: row.try_get("icon").map_err(backend)?,
        workspace_mode: text_enum(
            "teams",
            "workspace_mode",
            &get("workspace_mode")?,
            WorkspaceMode::parse,
        )?,
        layout: json("teams", "layout", &get("layout")?)?,
        archived_at: row.try_get("archived_at").map_err(backend)?,
        created_at: row.try_get("created_at").map_err(backend)?,
        updated_at: row.try_get("updated_at").map_err(backend)?,
    })
}

impl TeamRepository for Store {
    async fn create_team(&self, team: &Team) -> RepoResult<()> {
        sqlx::query(
            "INSERT INTO teams (id, name, mission, workdir, color, icon, workspace_mode, layout, \
             archived_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(team.id.as_str())
        .bind(&team.name)
        .bind(&team.mission)
        .bind(&team.workdir)
        .bind(team.color.as_str())
        .bind(&team.icon)
        .bind(team.workspace_mode.as_str())
        .bind(to_json(&team.layout)?)
        .bind(team.archived_at)
        .bind(team.created_at)
        .bind(team.updated_at)
        .execute(self.pool())
        .await
        .map_err(|e| {
            if is_primary_key_conflict(&e, "teams") {
                RepoError::AlreadyExists(team.id.to_string())
            } else {
                backend(e)
            }
        })?;
        Ok(())
    }

    async fn get_team(&self, id: &TeamId) -> RepoResult<Option<Team>> {
        let row = sqlx::query(&format!("SELECT {COLUMNS} FROM teams WHERE id = ?"))
            .bind(id.as_str())
            .fetch_optional(self.pool())
            .await
            .map_err(backend)?;
        row.as_ref().map(from_row).transpose()
    }

    async fn list_teams(&self, filter: TeamFilter) -> RepoResult<Vec<Team>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM teams WHERE (? OR archived_at IS NULL) ORDER BY created_at, id"
        );
        let rows = sqlx::query(&sql)
            .bind(filter.include_archived)
            .fetch_all(self.pool())
            .await
            .map_err(backend)?;
        rows.iter().map(from_row).collect()
    }

    async fn update_team(&self, team: &Team) -> RepoResult<()> {
        let done = sqlx::query(
            "UPDATE teams SET name = ?, mission = ?, workdir = ?, color = ?, icon = ?, \
             workspace_mode = ?, layout = ?, archived_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&team.name)
        .bind(&team.mission)
        .bind(&team.workdir)
        .bind(team.color.as_str())
        .bind(&team.icon)
        .bind(team.workspace_mode.as_str())
        .bind(to_json(&team.layout)?)
        .bind(team.archived_at)
        .bind(team.updated_at)
        .bind(team.id.as_str())
        .execute(self.pool())
        .await
        .map_err(backend)?;
        if done.rows_affected() == 0 {
            return Err(RepoError::TeamNotFound(team.id.clone()));
        }
        Ok(())
    }

    async fn set_team_archived(&self, id: &TeamId, archived_at: Option<Millis>) -> RepoResult<()> {
        let done = sqlx::query("UPDATE teams SET archived_at = ? WHERE id = ?")
            .bind(archived_at)
            .bind(id.as_str())
            .execute(self.pool())
            .await
            .map_err(backend)?;
        if done.rows_affected() == 0 {
            return Err(RepoError::TeamNotFound(id.clone()));
        }
        Ok(())
    }

    async fn delete_team(&self, id: &TeamId) -> RepoResult<()> {
        // Agentes, canais, mensagens e tarefas saem por ON DELETE CASCADE (I5).
        let done = sqlx::query("DELETE FROM teams WHERE id = ?")
            .bind(id.as_str())
            .execute(self.pool())
            .await
            .map_err(backend)?;
        if done.rows_affected() == 0 {
            return Err(RepoError::TeamNotFound(id.clone()));
        }
        Ok(())
    }
}
