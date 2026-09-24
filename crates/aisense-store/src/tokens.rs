//! `TokenRepository` sobre SQLite (tabela `agent_tokens`, F05-04).

use aisense_core::repo::{RepoError, RepoResult, TokenRecord, TokenRepository};
use aisense_core::{AgentId, Millis, SessionId};
use sqlx::Row;

use crate::convert::{backend, is_foreign_key_violation};
use crate::Store;

impl TokenRepository for Store {
    async fn insert_token(&self, token: &str, record: &TokenRecord) -> RepoResult<()> {
        sqlx::query(
            "INSERT INTO agent_tokens (token, agent_id, session_id, expires_at) VALUES (?, ?, ?, ?)",
        )
        .bind(token)
        .bind(record.agent_id.as_str())
        .bind(record.session_id.as_str())
        .bind(record.expires_at)
        .execute(self.pool())
        .await
        .map_err(|e| {
            if is_foreign_key_violation(&e) {
                RepoError::AgentNotFound(record.agent_id.clone())
            } else if e.as_database_error().is_some_and(|db| db.is_unique_violation()) {
                RepoError::AlreadyExists("token".into())
            } else {
                backend(e)
            }
        })?;
        Ok(())
    }

    async fn find_token(&self, token: &str, now: Millis) -> RepoResult<Option<TokenRecord>> {
        let row = sqlx::query(
            "SELECT agent_id, session_id, expires_at FROM agent_tokens \
             WHERE token = ? AND expires_at > ?",
        )
        .bind(token)
        .bind(now)
        .fetch_optional(self.pool())
        .await
        .map_err(backend)?;
        row.map(|row| {
            Ok(TokenRecord {
                agent_id: AgentId::from_raw(row.try_get::<String, _>("agent_id").map_err(backend)?),
                session_id: SessionId::from_raw(
                    row.try_get::<String, _>("session_id").map_err(backend)?,
                ),
                expires_at: row.try_get("expires_at").map_err(backend)?,
            })
        })
        .transpose()
    }

    async fn revoke_session_tokens(&self, session_id: &SessionId) -> RepoResult<u64> {
        Ok(sqlx::query("DELETE FROM agent_tokens WHERE session_id = ?")
            .bind(session_id.as_str())
            .execute(self.pool())
            .await
            .map_err(backend)?
            .rows_affected())
    }

    async fn revoke_all_tokens(&self) -> RepoResult<u64> {
        Ok(sqlx::query("DELETE FROM agent_tokens")
            .execute(self.pool())
            .await
            .map_err(backend)?
            .rows_affected())
    }
}
