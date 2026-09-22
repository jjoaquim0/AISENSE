//! `AgentRepository` sobre SQLite.

use aisense_core::agent::{Agent, Autonomy, DeliveryMode, Handle, RestartPolicy, Workbench};
use aisense_core::repo::{AgentRepository, RepoError, RepoResult};
use aisense_core::{AgentColor, AgentId, TeamId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::convert::{
    backend, corrupt, is_foreign_key_violation, is_handle_conflict, is_primary_key_conflict, json,
    text_enum, to_json,
};
use crate::Store;

const COLUMNS: &str = "id, team_id, handle, name, role, adapter_id, model, workdir, env, args, \
                       color, autostart, restart_policy, delivery_mode, autonomy, workbench, \
                       position, created_at, updated_at";

fn from_row(row: &SqliteRow) -> RepoResult<Agent> {
    let get = |col: &str| row.try_get::<String, _>(col).map_err(backend);
    let handle = get("handle")?;
    Ok(Agent {
        id: AgentId::from_raw(get("id")?),
        team_id: TeamId::from_raw(get("team_id")?),
        handle: Handle::parse(&handle).map_err(|_| corrupt("agents", "handle", &handle))?,
        name: get("name")?,
        role: get("role")?,
        adapter_id: get("adapter_id")?,
        model: row.try_get("model").map_err(backend)?,
        workdir: row.try_get("workdir").map_err(backend)?,
        env: json("agents", "env", &get("env")?)?,
        args: json("agents", "args", &get("args")?)?,
        color: text_enum("agents", "color", &get("color")?, AgentColor::parse)?,
        autostart: row.try_get("autostart").map_err(backend)?,
        restart_policy: text_enum(
            "agents",
            "restart_policy",
            &get("restart_policy")?,
            RestartPolicy::parse,
        )?,
        delivery_mode: text_enum(
            "agents",
            "delivery_mode",
            &get("delivery_mode")?,
            DeliveryMode::parse,
        )?,
        autonomy: text_enum("agents", "autonomy", &get("autonomy")?, Autonomy::parse)?,
        workbench: text_enum("agents", "workbench", &get("workbench")?, Workbench::parse)?,
        position: row.try_get("position").map_err(backend)?,
        created_at: row.try_get("created_at").map_err(backend)?,
        updated_at: row.try_get("updated_at").map_err(backend)?,
    })
}

fn write_error(agent: &Agent, err: sqlx::Error) -> RepoError {
    if is_handle_conflict(&err) {
        RepoError::DuplicateHandle(agent.handle.as_str().to_owned())
    } else if is_primary_key_conflict(&err, "agents") {
        RepoError::AlreadyExists(agent.id.to_string())
    } else if is_foreign_key_violation(&err) {
        RepoError::TeamNotFound(agent.team_id.clone())
    } else {
        backend(err)
    }
}

impl AgentRepository for Store {
    async fn create_agent(&self, agent: &Agent) -> RepoResult<()> {
        sqlx::query(
            "INSERT INTO agents (id, team_id, handle, name, role, adapter_id, model, workdir, env, \
             args, color, autostart, restart_policy, delivery_mode, autonomy, workbench, position, \
             created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(agent.id.as_str())
        .bind(agent.team_id.as_str())
        .bind(agent.handle.as_str())
        .bind(&agent.name)
        .bind(&agent.role)
        .bind(&agent.adapter_id)
        .bind(&agent.model)
        .bind(&agent.workdir)
        .bind(to_json(&agent.env)?)
        .bind(to_json(&agent.args)?)
        .bind(agent.color.as_str())
        .bind(agent.autostart)
        .bind(agent.restart_policy.as_str())
        .bind(agent.delivery_mode.as_str())
        .bind(agent.autonomy.as_str())
        .bind(agent.workbench.as_str())
        .bind(agent.position)
        .bind(agent.created_at)
        .bind(agent.updated_at)
        .execute(self.pool())
        .await
        .map_err(|e| write_error(agent, e))?;
        Ok(())
    }

    async fn get_agent(&self, id: &AgentId) -> RepoResult<Option<Agent>> {
        let row = sqlx::query(&format!("SELECT {COLUMNS} FROM agents WHERE id = ?"))
            .bind(id.as_str())
            .fetch_optional(self.pool())
            .await
            .map_err(backend)?;
        row.as_ref().map(from_row).transpose()
    }

    async fn find_agent_by_handle(
        &self,
        team_id: &TeamId,
        handle: &Handle,
    ) -> RepoResult<Option<Agent>> {
        let row = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM agents WHERE team_id = ? AND handle = ?"
        ))
        .bind(team_id.as_str())
        .bind(handle.as_str())
        .fetch_optional(self.pool())
        .await
        .map_err(backend)?;
        row.as_ref().map(from_row).transpose()
    }

    async fn list_agents(&self, team_id: &TeamId) -> RepoResult<Vec<Agent>> {
        let rows = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM agents WHERE team_id = ? ORDER BY position, created_at, id"
        ))
        .bind(team_id.as_str())
        .fetch_all(self.pool())
        .await
        .map_err(backend)?;
        rows.iter().map(from_row).collect()
    }

    async fn update_agent(&self, agent: &Agent) -> RepoResult<()> {
        // team_id e created_at não mudam: mover agente entre equipes não é edição.
        let done = sqlx::query(
            "UPDATE agents SET handle = ?, name = ?, role = ?, adapter_id = ?, model = ?, \
             workdir = ?, env = ?, args = ?, color = ?, autostart = ?, restart_policy = ?, \
             delivery_mode = ?, autonomy = ?, workbench = ?, position = ?, updated_at = ? \
             WHERE id = ?",
        )
        .bind(agent.handle.as_str())
        .bind(&agent.name)
        .bind(&agent.role)
        .bind(&agent.adapter_id)
        .bind(&agent.model)
        .bind(&agent.workdir)
        .bind(to_json(&agent.env)?)
        .bind(to_json(&agent.args)?)
        .bind(agent.color.as_str())
        .bind(agent.autostart)
        .bind(agent.restart_policy.as_str())
        .bind(agent.delivery_mode.as_str())
        .bind(agent.autonomy.as_str())
        .bind(agent.workbench.as_str())
        .bind(agent.position)
        .bind(agent.updated_at)
        .bind(agent.id.as_str())
        .execute(self.pool())
        .await
        .map_err(|e| write_error(agent, e))?;
        if done.rows_affected() == 0 {
            return Err(RepoError::AgentNotFound(agent.id.clone()));
        }
        Ok(())
    }

    async fn delete_agent(&self, id: &AgentId) -> RepoResult<()> {
        let done = sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(id.as_str())
            .execute(self.pool())
            .await
            .map_err(backend)?;
        if done.rows_affected() == 0 {
            return Err(RepoError::AgentNotFound(id.clone()));
        }
        Ok(())
    }
}
