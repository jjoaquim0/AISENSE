//! `ProposalRepository` sobre SQLite (tabela `proposals`, F07-02).

use aisense_core::proposal::{Proposal, ProposalAction, ProposalRepository, ProposalState};
use aisense_core::repo::{RepoError, RepoResult};
use aisense_core::{AgentId, Millis, ProposalId, TeamId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::convert::{backend, is_foreign_key_violation, json, text_enum, to_json};
use crate::Store;

fn from_row(row: &SqliteRow) -> RepoResult<Proposal> {
    let text = |col: &str| row.try_get::<String, _>(col).map_err(backend);
    let state = text("state")?;
    Ok(Proposal {
        id: ProposalId::from_raw(text("id")?),
        team_id: TeamId::from_raw(text("team_id")?),
        proposed_by: row
            .try_get::<Option<String>, _>("proposed_by")
            .map_err(backend)?
            .map(AgentId::from_raw),
        action: json::<ProposalAction>("proposals", "action", &text("action")?)?,
        reason: text("reason")?,
        state: text_enum("proposals", "state", &state, ProposalState::parse)?,
        created_at: row.try_get("created_at").map_err(backend)?,
        decided_at: row.try_get("decided_at").map_err(backend)?,
        decision_note: row.try_get("decision_note").map_err(backend)?,
    })
}

impl ProposalRepository for Store {
    async fn insert_proposal(&self, p: &Proposal) -> RepoResult<()> {
        sqlx::query(
            "INSERT INTO proposals (id, team_id, proposed_by, action, reason, state, created_at, \
             decided_at, decision_note) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(p.id.as_str())
        .bind(p.team_id.as_str())
        .bind(p.proposed_by.as_ref().map(AgentId::as_str))
        .bind(to_json(&p.action)?)
        .bind(&p.reason)
        .bind(p.state.as_str())
        .bind(p.created_at)
        .bind(p.decided_at)
        .bind(&p.decision_note)
        .execute(self.pool())
        .await
        .map_err(|e| {
            if is_foreign_key_violation(&e) {
                RepoError::TeamNotFound(p.team_id.clone())
            } else {
                backend(e)
            }
        })?;
        Ok(())
    }

    async fn get_proposal(&self, id: &ProposalId) -> RepoResult<Option<Proposal>> {
        sqlx::query("SELECT * FROM proposals WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(self.pool())
            .await
            .map_err(backend)?
            .as_ref()
            .map(from_row)
            .transpose()
    }

    async fn list_proposals(
        &self,
        team_id: &TeamId,
        pending_only: bool,
    ) -> RepoResult<Vec<Proposal>> {
        sqlx::query(
            "SELECT * FROM proposals WHERE team_id = ? AND (NOT ? OR state = 'pending') \
             ORDER BY id DESC",
        )
        .bind(team_id.as_str())
        .bind(pending_only)
        .fetch_all(self.pool())
        .await
        .map_err(backend)?
        .iter()
        .map(from_row)
        .collect()
    }

    async fn decide_proposal(
        &self,
        id: &ProposalId,
        state: ProposalState,
        now: Millis,
        note: Option<&str>,
    ) -> RepoResult<bool> {
        let done = sqlx::query(
            "UPDATE proposals SET state = ?, decided_at = ?, decision_note = ? \
             WHERE id = ? AND state = 'pending'",
        )
        .bind(state.as_str())
        .bind(now)
        .bind(note)
        .bind(id.as_str())
        .execute(self.pool())
        .await
        .map_err(backend)?;
        Ok(done.rows_affected() == 1)
    }
}
