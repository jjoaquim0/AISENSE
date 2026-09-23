//! `SessionRepository` sobre SQLite (tabela `sessions`).

use aisense_core::repo::{
    RepoError, RepoResult, SessionRecord, SessionRepository, SESSIONS_KEPT_PER_AGENT,
};
use aisense_core::{AgentId, Millis, SessionId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::convert::{backend, is_foreign_key_violation, is_primary_key_conflict};
use crate::Store;

fn from_row(row: &SqliteRow) -> RepoResult<SessionRecord> {
    let pid: Option<i64> = row.try_get("pid").map_err(backend)?;
    Ok(SessionRecord {
        id: SessionId::from_raw(row.try_get::<String, _>("id").map_err(backend)?),
        agent_id: AgentId::from_raw(row.try_get::<String, _>("agent_id").map_err(backend)?),
        pid: pid.and_then(|p| u32::try_from(p).ok()),
        started_at: row.try_get("started_at").map_err(backend)?,
        ended_at: row.try_get("ended_at").map_err(backend)?,
        exit_code: row.try_get("exit_code").map_err(backend)?,
        log_path: row.try_get("log_path").map_err(backend)?,
    })
}

impl SessionRepository for Store {
    async fn start_session(&self, session: &SessionRecord) -> RepoResult<()> {
        let mut tx = self.pool().begin().await.map_err(backend)?;
        sqlx::query(
            "INSERT INTO sessions (id, agent_id, pid, started_at, ended_at, exit_code, log_path) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(session.id.as_str())
        .bind(session.agent_id.as_str())
        .bind(session.pid.map(i64::from))
        .bind(session.started_at)
        .bind(session.ended_at)
        .bind(session.exit_code)
        .bind(&session.log_path)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            if is_primary_key_conflict(&err, "sessions") {
                RepoError::AlreadyExists(session.id.to_string())
            } else if is_foreign_key_violation(&err) {
                RepoError::AgentNotFound(session.agent_id.clone())
            } else {
                backend(err)
            }
        })?;

        // Retenção (`docs/04`): só as últimas execuções de cada agente.
        let keep = i64::try_from(SESSIONS_KEPT_PER_AGENT).unwrap_or(i64::MAX);
        sqlx::query(
            "DELETE FROM sessions WHERE agent_id = ? AND id NOT IN \
             (SELECT id FROM sessions WHERE agent_id = ? ORDER BY started_at DESC, id DESC LIMIT ?)",
        )
        .bind(session.agent_id.as_str())
        .bind(session.agent_id.as_str())
        .bind(keep)
        .execute(&mut *tx)
        .await
        .map_err(backend)?;
        tx.commit().await.map_err(backend)
    }

    async fn end_session(
        &self,
        id: &SessionId,
        ended_at: Millis,
        exit_code: Option<i32>,
    ) -> RepoResult<()> {
        sqlx::query("UPDATE sessions SET ended_at = ?, exit_code = ? WHERE id = ?")
            .bind(ended_at)
            .bind(exit_code)
            .bind(id.as_str())
            .execute(self.pool())
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn list_sessions(
        &self,
        agent_id: &AgentId,
        limit: usize,
    ) -> RepoResult<Vec<SessionRecord>> {
        let rows = sqlx::query(
            "SELECT id, agent_id, pid, started_at, ended_at, exit_code, log_path FROM sessions \
             WHERE agent_id = ? ORDER BY started_at DESC, id DESC LIMIT ?",
        )
        .bind(agent_id.as_str())
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(self.pool())
        .await
        .map_err(backend)?;
        rows.iter().map(from_row).collect()
    }
}
