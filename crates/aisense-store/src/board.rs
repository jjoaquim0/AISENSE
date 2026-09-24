//! `BoardRepository` sobre SQLite (tabelas `boards`, `columns`, `tasks`,
//! `task_dependencies`, `task_comments`, `task_activity`; F06-02).

use aisense_core::board::{
    Activity, Actor, Automation, Board, BoardRepository, Card, CardPriority, CardQuery, CardWrite,
    Column, ColumnKind, Comment, WriteGuard,
};
use aisense_core::repo::{RepoError, RepoResult};
use aisense_core::{ActivityId, AgentId, BoardId, CardId, ColumnId, CommentId, Millis, TeamId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::convert::{backend, corrupt, is_foreign_key_violation, json, text_enum, to_json};
use crate::Store;

const CARD_COLUMNS: &str = "id, team_id, column_id, title, body, assignee, created_by, parent_id, \
     position, priority, labels, checklist, links, block_reason, version, archived_at, \
     approved_by, approved_at, column_since, created_at, updated_at";

fn card_from_row(row: &SqliteRow) -> RepoResult<Card> {
    let text = |col: &str| row.try_get::<String, _>(col).map_err(backend);
    let opt = |col: &str| row.try_get::<Option<String>, _>(col).map_err(backend);
    let int = |col: &str| row.try_get::<i64, _>(col).map_err(backend);
    let id = text("id")?;
    let priority = text("priority")?;
    Ok(Card {
        column_id: ColumnId::from_raw(
            opt("column_id")?.ok_or_else(|| corrupt("tasks", "column_id", &id))?,
        ),
        id: CardId::from_raw(id),
        team_id: TeamId::from_raw(text("team_id")?),
        title: text("title")?,
        body: text("body")?,
        assignee: opt("assignee")?.map(AgentId::from_raw),
        created_by: opt("created_by")?.map(AgentId::from_raw),
        parent_id: opt("parent_id")?.map(CardId::from_raw),
        position: int("position")?,
        priority: text_enum("tasks", "priority", &priority, CardPriority::parse)?,
        labels: json("tasks", "labels", &text("labels")?)?,
        checklist: json("tasks", "checklist", &text("checklist")?)?,
        links: json("tasks", "links", &text("links")?)?,
        block_reason: opt("block_reason")?,
        version: int("version")?,
        archived_at: row.try_get("archived_at").map_err(backend)?,
        approved_by: opt("approved_by")?.map(AgentId::from_raw),
        approved_at: row.try_get("approved_at").map_err(backend)?,
        column_since: int("column_since")?,
        created_at: int("created_at")?,
        updated_at: int("updated_at")?,
    })
}

fn column_from_row(row: &SqliteRow) -> RepoResult<Column> {
    let text = |col: &str| row.try_get::<String, _>(col).map_err(backend);
    let limit = |col: &str| -> RepoResult<Option<u32>> {
        Ok(row
            .try_get::<Option<i64>, _>(col)
            .map_err(backend)?
            .and_then(|v| u32::try_from(v).ok()))
    };
    let kind = text("kind")?;
    Ok(Column {
        id: ColumnId::from_raw(text("id")?),
        board_id: BoardId::from_raw(text("board_id")?),
        slug: text("slug")?,
        name: text("name")?,
        kind: text_enum("columns", "kind", &kind, ColumnKind::parse)?,
        wip_limit: limit("wip_limit")?,
        wip_per_agent: limit("wip_per_agent")?,
        position: u32::try_from(row.try_get::<i64, _>("position").map_err(backend)?)
            .unwrap_or(u32::MAX),
        requires_approval: row.try_get("requires_approval").map_err(backend)?,
        approver_must_differ: row.try_get("approver_must_differ").map_err(backend)?,
        requires_commands: json("columns", "requires_commands", &text("requires_commands")?)?,
    })
}

fn actor_from_row(row: &SqliteRow, table: &str) -> RepoResult<Actor> {
    let kind = row.try_get::<String, _>("author_kind").map_err(backend)?;
    let author = row
        .try_get::<Option<String>, _>("author")
        .map_err(backend)?;
    Actor::from_columns(&kind, author).ok_or_else(|| corrupt(table, "author_kind", &kind))
}

fn comment_from_row(row: &SqliteRow) -> RepoResult<Comment> {
    Ok(Comment {
        id: CommentId::from_raw(row.try_get::<String, _>("id").map_err(backend)?),
        card_id: CardId::from_raw(row.try_get::<String, _>("task_id").map_err(backend)?),
        author: actor_from_row(row, "task_comments")?,
        body: row.try_get("body").map_err(backend)?,
        created_at: row.try_get("created_at").map_err(backend)?,
    })
}

fn activity_from_row(row: &SqliteRow) -> RepoResult<Activity> {
    let detail = row.try_get::<String, _>("detail").map_err(backend)?;
    Ok(Activity {
        id: ActivityId::from_raw(row.try_get::<String, _>("id").map_err(backend)?),
        card_id: CardId::from_raw(row.try_get::<String, _>("task_id").map_err(backend)?),
        actor: actor_from_row(row, "task_activity")?,
        action: row.try_get("action").map_err(backend)?,
        detail: json("task_activity", "detail", &detail)?,
        created_at: row.try_get("created_at").map_err(backend)?,
    })
}

async fn insert_column<'e, E>(executor: E, column: &Column) -> RepoResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO columns (id, board_id, slug, name, kind, wip_limit, wip_per_agent, position, \
         requires_approval, approver_must_differ, requires_commands) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET slug = excluded.slug, name = excluded.name, \
         kind = excluded.kind, wip_limit = excluded.wip_limit, \
         wip_per_agent = excluded.wip_per_agent, position = excluded.position, \
         requires_approval = excluded.requires_approval, \
         approver_must_differ = excluded.approver_must_differ, \
         requires_commands = excluded.requires_commands",
    )
    .bind(column.id.as_str())
    .bind(column.board_id.as_str())
    .bind(&column.slug)
    .bind(&column.name)
    .bind(column.kind.as_str())
    .bind(column.wip_limit.map(i64::from))
    .bind(column.wip_per_agent.map(i64::from))
    .bind(i64::from(column.position))
    .bind(column.requires_approval)
    .bind(column.approver_must_differ)
    .bind(to_json(&column.requires_commands)?)
    .execute(executor)
    .await
    .map_err(backend)?;
    Ok(())
}

impl BoardRepository for Store {
    async fn create_board(&self, board: &Board, columns: &[Column]) -> RepoResult<()> {
        let mut tx = self.pool().begin().await.map_err(backend)?;
        sqlx::query(
            "INSERT INTO boards (id, team_id, automations, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(board.id.as_str())
        .bind(board.team_id.as_str())
        .bind(to_json(&board.automations)?)
        .bind(board.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if e.as_database_error()
                .is_some_and(|db| db.is_unique_violation())
            {
                RepoError::AlreadyExists(format!("board of {}", board.team_id))
            } else if is_foreign_key_violation(&e) {
                RepoError::TeamNotFound(board.team_id.clone())
            } else {
                backend(e)
            }
        })?;
        for column in columns {
            insert_column(&mut *tx, column).await?;
        }
        // Tarefas de antes do quadro (0001 tinha `status`): cada uma vai para a coluna de
        // mesmo slug, ou para a primeira `ready`.
        sqlx::query(
            "UPDATE tasks SET \
               column_id = COALESCE( \
                 (SELECT c.id FROM columns c WHERE c.board_id = ?1 AND c.slug = \
                    CASE tasks.status WHEN 'cancelled' THEN 'done' ELSE tasks.status END), \
                 (SELECT c.id FROM columns c WHERE c.board_id = ?1 AND c.kind = 'ready' \
                    ORDER BY c.position LIMIT 1)), \
               column_since = updated_at \
             WHERE team_id = ?2 AND column_id IS NULL",
        )
        .bind(board.id.as_str())
        .bind(board.team_id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(backend)?;
        tx.commit().await.map_err(backend)?;
        Ok(())
    }

    async fn get_board(&self, team_id: &TeamId) -> RepoResult<Option<Board>> {
        let Some(row) = sqlx::query("SELECT * FROM boards WHERE team_id = ?")
            .bind(team_id.as_str())
            .fetch_optional(self.pool())
            .await
            .map_err(backend)?
        else {
            return Ok(None);
        };
        let automations = row.try_get::<String, _>("automations").map_err(backend)?;
        Ok(Some(Board {
            id: BoardId::from_raw(row.try_get::<String, _>("id").map_err(backend)?),
            team_id: team_id.clone(),
            automations: json::<Vec<Automation>>("boards", "automations", &automations)?,
            created_at: row.try_get("created_at").map_err(backend)?,
        }))
    }

    async fn set_automations(
        &self,
        board_id: &BoardId,
        automations: &[Automation],
    ) -> RepoResult<()> {
        sqlx::query("UPDATE boards SET automations = ? WHERE id = ?")
            .bind(to_json(&automations)?)
            .bind(board_id.as_str())
            .execute(self.pool())
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn list_columns(&self, board_id: &BoardId) -> RepoResult<Vec<Column>> {
        sqlx::query("SELECT * FROM columns WHERE board_id = ? ORDER BY position, slug")
            .bind(board_id.as_str())
            .fetch_all(self.pool())
            .await
            .map_err(backend)?
            .iter()
            .map(column_from_row)
            .collect()
    }

    async fn replace_columns(
        &self,
        board_id: &BoardId,
        columns: &[Column],
        moves: &[(ColumnId, ColumnId)],
        now: Millis,
    ) -> RepoResult<()> {
        let mut tx = self.pool().begin().await.map_err(backend)?;
        for (from, to) in moves {
            sqlx::query(
                "UPDATE tasks SET column_id = ?, column_since = ?, updated_at = ?, \
                 version = version + 1 WHERE column_id = ?",
            )
            .bind(to.as_str())
            .bind(now)
            .bind(now)
            .bind(from.as_str())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        }
        // Slugs temporários: trocar `a` com `b` não esbarra no UNIQUE no meio do caminho.
        sqlx::query("UPDATE columns SET slug = '~' || id WHERE board_id = ?")
            .bind(board_id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        for column in columns {
            insert_column(&mut *tx, column).await?;
        }
        let kept: Vec<&str> = columns.iter().map(|c| c.id.as_str()).collect();
        sqlx::query(
            "DELETE FROM columns WHERE board_id = ? AND id NOT IN (SELECT value FROM json_each(?))",
        )
        .bind(board_id.as_str())
        .bind(to_json(&kept)?)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if is_foreign_key_violation(&e) {
                RepoError::Corrupt("a removed column still has cards".into())
            } else {
                backend(e)
            }
        })?;
        tx.commit().await.map_err(backend)?;
        Ok(())
    }

    async fn insert_card(&self, card: &Card) -> RepoResult<()> {
        sqlx::query(&format!(
            "INSERT INTO tasks ({CARD_COLUMNS}) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        ))
        .bind(card.id.as_str())
        .bind(card.team_id.as_str())
        .bind(card.column_id.as_str())
        .bind(&card.title)
        .bind(&card.body)
        .bind(card.assignee.as_ref().map(AgentId::as_str))
        .bind(card.created_by.as_ref().map(AgentId::as_str))
        .bind(card.parent_id.as_ref().map(CardId::as_str))
        .bind(card.position)
        .bind(card.priority.as_str())
        .bind(to_json(&card.labels)?)
        .bind(to_json(&card.checklist)?)
        .bind(to_json(&card.links)?)
        .bind(&card.block_reason)
        .bind(card.version)
        .bind(card.archived_at)
        .bind(card.approved_by.as_ref().map(AgentId::as_str))
        .bind(card.approved_at)
        .bind(card.column_since)
        .bind(card.created_at)
        .bind(card.updated_at)
        .execute(self.pool())
        .await
        .map_err(|e| {
            if crate::convert::is_primary_key_conflict(&e, "tasks") {
                RepoError::AlreadyExists(card.id.to_string())
            } else if is_foreign_key_violation(&e) {
                RepoError::TeamNotFound(card.team_id.clone())
            } else {
                backend(e)
            }
        })?;
        Ok(())
    }

    async fn get_card(&self, id: &CardId) -> RepoResult<Option<Card>> {
        sqlx::query(&format!(
            "SELECT {CARD_COLUMNS} FROM tasks WHERE id = ? AND column_id IS NOT NULL"
        ))
        .bind(id.as_str())
        .fetch_optional(self.pool())
        .await
        .map_err(backend)?
        .as_ref()
        .map(card_from_row)
        .transpose()
    }

    async fn list_cards(&self, team_id: &TeamId, query: &CardQuery) -> RepoResult<Vec<Card>> {
        // O SQLite monta um JSON só com todos os cartões: decodificar 1.000 linhas de 21
        // colunas pelo driver custa mais que o próprio SELECT (aceite: 1.000 em <20 ms).
        let raw: Option<String> = sqlx::query_scalar(
            "SELECT json_group_array(json_object( \
               'id', t.id, 'teamId', t.team_id, 'columnId', t.column_id, 'title', t.title, \
               'body', t.body, 'assignee', t.assignee, 'createdBy', t.created_by, \
               'parentId', t.parent_id, 'position', t.position, 'priority', t.priority, \
               'labels', json(t.labels), 'checklist', json(t.checklist), 'links', json(t.links), \
               'blockReason', t.block_reason, 'version', t.version, 'archivedAt', t.archived_at, \
               'approvedBy', t.approved_by, 'approvedAt', t.approved_at, \
               'columnSince', t.column_since, 'createdAt', t.created_at, 'updatedAt', t.updated_at)) \
             FROM (SELECT t.* FROM tasks t JOIN columns c ON c.id = t.column_id \
               WHERE t.team_id = ?1 \
                 AND (?2 OR t.archived_at IS NULL) \
                 AND (?3 IS NULL OR t.column_id = ?3) \
                 AND (?4 IS NULL OR t.assignee = ?4) \
                 AND (NOT ?5 OR t.assignee IS NULL) \
                 AND (?6 IS NULL OR EXISTS (SELECT 1 FROM json_each(t.labels) WHERE value = ?6)) \
               ORDER BY c.position, t.position, t.id) t",
        )
        .bind(team_id.as_str())
        .bind(query.include_archived)
        .bind(query.column_id.as_ref().map(ColumnId::as_str))
        .bind(query.assignee.as_ref().map(AgentId::as_str))
        .bind(query.unassigned)
        .bind(query.label.as_deref())
        .fetch_one(self.pool())
        .await
        .map_err(backend)?;
        match raw {
            None => Ok(Vec::new()),
            Some(raw) => serde_json::from_str(&raw)
                .map_err(|e| RepoError::Corrupt(format!("tasks of team {team_id}: {e}"))),
        }
    }

    async fn update_card(&self, card: &Card, guard: &WriteGuard) -> RepoResult<CardWrite> {
        // Uma instrução só: versão, "ninguém pegou" e WIP são conferidos no mesmo instante em
        // que se grava. Duas gravações concorrentes não passam as duas (F06-03).
        let done = sqlx::query(
            "UPDATE tasks SET column_id = ?1, title = ?2, body = ?3, assignee = ?4, \
               parent_id = ?5, position = ?6, priority = ?7, labels = ?8, checklist = ?9, \
               links = ?10, block_reason = ?11, version = ?12, archived_at = ?13, \
               approved_by = ?14, approved_at = ?15, column_since = ?16, updated_at = ?17 \
             WHERE id = ?18 AND version = ?19 \
               AND (NOT ?20 OR assignee IS NULL) \
               AND (?21 IS NULL OR (SELECT COUNT(*) FROM tasks o WHERE o.column_id = ?1 \
                    AND o.archived_at IS NULL AND o.id <> ?18) < ?21) \
               AND (?22 IS NULL OR ?4 IS NULL OR (SELECT COUNT(*) FROM tasks o \
                    WHERE o.column_id = ?1 AND o.assignee = ?4 AND o.archived_at IS NULL \
                    AND o.id <> ?18) < ?22)",
        )
        .bind(card.column_id.as_str())
        .bind(&card.title)
        .bind(&card.body)
        .bind(card.assignee.as_ref().map(AgentId::as_str))
        .bind(card.parent_id.as_ref().map(CardId::as_str))
        .bind(card.position)
        .bind(card.priority.as_str())
        .bind(to_json(&card.labels)?)
        .bind(to_json(&card.checklist)?)
        .bind(to_json(&card.links)?)
        .bind(&card.block_reason)
        .bind(card.version)
        .bind(card.archived_at)
        .bind(card.approved_by.as_ref().map(AgentId::as_str))
        .bind(card.approved_at)
        .bind(card.column_since)
        .bind(card.updated_at)
        .bind(card.id.as_str())
        .bind(guard.expected_version)
        .bind(guard.require_unassigned)
        .bind(guard.wip_limit.map(i64::from))
        .bind(guard.wip_per_agent.map(i64::from))
        .execute(self.pool())
        .await
        .map_err(backend)?;
        if done.rows_affected() == 1 {
            return Ok(CardWrite::Written);
        }
        let current: Option<(i64, Option<String>)> =
            sqlx::query_as("SELECT version, assignee FROM tasks WHERE id = ?")
                .bind(card.id.as_str())
                .fetch_optional(self.pool())
                .await
                .map_err(backend)?;
        match current {
            None => Err(RepoError::Corrupt(format!("card {} not found", card.id))),
            Some((version, assignee))
                if version != guard.expected_version
                    || (guard.require_unassigned && assignee.is_some()) =>
            {
                Ok(CardWrite::Stale)
            }
            Some(_) => Ok(CardWrite::WipFull),
        }
    }

    async fn dependencies(&self, team_id: &TeamId) -> RepoResult<Vec<(CardId, CardId)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT d.task_id, d.depends_on FROM task_dependencies d \
             JOIN tasks t ON t.id = d.task_id WHERE t.team_id = ? ORDER BY d.task_id, d.depends_on",
        )
        .bind(team_id.as_str())
        .fetch_all(self.pool())
        .await
        .map_err(backend)?;
        Ok(rows
            .into_iter()
            .map(|(t, d)| (CardId::from_raw(t), CardId::from_raw(d)))
            .collect())
    }

    async fn add_dependency(&self, task: &CardId, depends_on: &CardId) -> RepoResult<()> {
        sqlx::query(
            "INSERT INTO task_dependencies (task_id, depends_on) VALUES (?, ?) \
             ON CONFLICT (task_id, depends_on) DO NOTHING",
        )
        .bind(task.as_str())
        .bind(depends_on.as_str())
        .execute(self.pool())
        .await
        .map_err(|e| {
            if is_foreign_key_violation(&e)
                || e.as_database_error()
                    .is_some_and(|db| db.message().contains("CHECK"))
            {
                RepoError::Corrupt(format!("dependency {task} -> {depends_on}"))
            } else {
                backend(e)
            }
        })?;
        Ok(())
    }

    async fn remove_dependency(&self, task: &CardId, depends_on: &CardId) -> RepoResult<()> {
        sqlx::query("DELETE FROM task_dependencies WHERE task_id = ? AND depends_on = ?")
            .bind(task.as_str())
            .bind(depends_on.as_str())
            .execute(self.pool())
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn insert_comment(&self, comment: &Comment) -> RepoResult<()> {
        let (kind, author) = comment.author.to_columns();
        sqlx::query(
            "INSERT INTO task_comments (id, task_id, author_kind, author, body, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(comment.id.as_str())
        .bind(comment.card_id.as_str())
        .bind(kind)
        .bind(author)
        .bind(&comment.body)
        .bind(comment.created_at)
        .execute(self.pool())
        .await
        .map_err(|e| {
            if is_foreign_key_violation(&e) {
                RepoError::Corrupt(format!("card {} not found", comment.card_id))
            } else {
                backend(e)
            }
        })?;
        Ok(())
    }

    async fn comment_counts(&self, team_id: &TeamId) -> RepoResult<Vec<(CardId, u32)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT c.task_id, COUNT(*) FROM task_comments c JOIN tasks t ON t.id = c.task_id \
             WHERE t.team_id = ? GROUP BY c.task_id ORDER BY c.task_id",
        )
        .bind(team_id.as_str())
        .fetch_all(self.pool())
        .await
        .map_err(backend)?;
        Ok(rows
            .into_iter()
            .map(|(id, n)| (CardId::from_raw(id), u32::try_from(n).unwrap_or(u32::MAX)))
            .collect())
    }

    async fn list_comments(&self, card_id: &CardId) -> RepoResult<Vec<Comment>> {
        sqlx::query("SELECT * FROM task_comments WHERE task_id = ? ORDER BY created_at, id")
            .bind(card_id.as_str())
            .fetch_all(self.pool())
            .await
            .map_err(backend)?
            .iter()
            .map(comment_from_row)
            .collect()
    }

    async fn insert_activity(&self, activity: &Activity) -> RepoResult<()> {
        let (kind, author) = activity.actor.to_columns();
        sqlx::query(
            "INSERT INTO task_activity (id, task_id, author_kind, author, action, detail, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(activity.id.as_str())
        .bind(activity.card_id.as_str())
        .bind(kind)
        .bind(author)
        .bind(&activity.action)
        .bind(to_json(&activity.detail)?)
        .bind(activity.created_at)
        .execute(self.pool())
        .await
        .map_err(|e| {
            if is_foreign_key_violation(&e) {
                RepoError::Corrupt(format!("card {} not found", activity.card_id))
            } else {
                backend(e)
            }
        })?;
        Ok(())
    }

    async fn list_activity(&self, card_id: &CardId) -> RepoResult<Vec<Activity>> {
        sqlx::query("SELECT * FROM task_activity WHERE task_id = ? ORDER BY created_at, id")
            .bind(card_id.as_str())
            .fetch_all(self.pool())
            .await
            .map_err(backend)?
            .iter()
            .map(activity_from_row)
            .collect()
    }

    async fn activity_since(&self, team_id: &TeamId, since: Millis) -> RepoResult<Vec<Activity>> {
        sqlx::query(
            "SELECT a.* FROM task_activity a JOIN tasks t ON t.id = a.task_id \
             WHERE t.team_id = ? AND a.created_at > ? ORDER BY a.created_at, a.id",
        )
        .bind(team_id.as_str())
        .bind(since)
        .fetch_all(self.pool())
        .await
        .map_err(backend)?
        .iter()
        .map(activity_from_row)
        .collect()
    }
}
