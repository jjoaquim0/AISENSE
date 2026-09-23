//! `SkillRepository` sobre SQLite (tabelas `skills` e `agent_skills`).

use aisense_core::repo::{
    dedup_agent_skills, skill_origin, AgentSkill, RepoError, RepoResult, SkillRecord,
    SkillRepository,
};
use aisense_core::skill::Skill;
use aisense_core::{AgentId, Millis, SkillId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::convert::{backend, json, to_json};
use crate::Store;

const COLUMNS: &str =
    "id, slug, description, version, source, path, targets, created_at, updated_at";

fn from_row(row: &SqliteRow) -> RepoResult<SkillRecord> {
    let get = |col: &str| row.try_get::<String, _>(col).map_err(backend);
    Ok(SkillRecord {
        id: SkillId::from_raw(get("id")?),
        slug: get("slug")?,
        description: get("description")?,
        version: get("version")?,
        source: get("source")?,
        path: get("path")?,
        targets: json("skills", "targets", &get("targets")?)?,
        created_at: row.try_get("created_at").map_err(backend)?,
        updated_at: row.try_get("updated_at").map_err(backend)?,
    })
}

async fn all(executor: impl sqlx::SqliteExecutor<'_>) -> RepoResult<Vec<SkillRecord>> {
    let rows = sqlx::query(&format!("SELECT {COLUMNS} FROM skills ORDER BY slug"))
        .fetch_all(executor)
        .await
        .map_err(backend)?;
    rows.iter().map(from_row).collect()
}

impl SkillRepository for Store {
    async fn sync_skills(&self, skills: &[Skill], now: Millis) -> RepoResult<Vec<SkillRecord>> {
        let mut tx = self.pool().begin().await.map_err(backend)?;
        let known = all(&mut *tx).await?;
        for skill in skills {
            let (source, path) = skill_origin(skill);
            let targets = to_json(&skill.targets)?;
            match known.iter().find(|r| r.slug == skill.name) {
                Some(record) if record.matches(skill) => {}
                Some(record) => {
                    sqlx::query(
                        "UPDATE skills SET description = ?, version = ?, source = ?, path = ?, \
                         targets = ?, updated_at = ? WHERE id = ?",
                    )
                    .bind(&skill.description)
                    .bind(&skill.version)
                    .bind(source)
                    .bind(&path)
                    .bind(&targets)
                    .bind(now)
                    .bind(record.id.as_str())
                    .execute(&mut *tx)
                    .await
                    .map_err(backend)?;
                }
                None => {
                    // `name` é a coluna de exibição do esquema; o nome da skill é o slug.
                    sqlx::query(
                        "INSERT INTO skills (id, slug, name, description, version, source, path, \
                         targets, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(SkillId::new().as_str())
                    .bind(&skill.name)
                    .bind(&skill.name)
                    .bind(&skill.description)
                    .bind(&skill.version)
                    .bind(source)
                    .bind(&path)
                    .bind(&targets)
                    .bind(now)
                    .bind(now)
                    .execute(&mut *tx)
                    .await
                    .map_err(backend)?;
                }
            }
        }
        let synced = all(&mut *tx).await?;
        tx.commit().await.map_err(backend)?;
        Ok(synced)
    }

    async fn list_skills(&self) -> RepoResult<Vec<SkillRecord>> {
        all(self.pool()).await
    }

    async fn agent_skills(&self, agent_id: &AgentId) -> RepoResult<Vec<AgentSkill>> {
        let rows = sqlx::query(
            "SELECT skill_id, enabled FROM agent_skills WHERE agent_id = ? \
             ORDER BY position, rowid",
        )
        .bind(agent_id.as_str())
        .fetch_all(self.pool())
        .await
        .map_err(backend)?;
        rows.iter()
            .map(|row| {
                Ok(AgentSkill {
                    skill_id: SkillId::from_raw(
                        row.try_get::<String, _>("skill_id").map_err(backend)?,
                    ),
                    enabled: row.try_get("enabled").map_err(backend)?,
                })
            })
            .collect()
    }

    async fn set_agent_skills(&self, agent_id: &AgentId, skills: &[AgentSkill]) -> RepoResult<()> {
        let mut tx = self.pool().begin().await.map_err(backend)?;
        let agent: Option<(String,)> = sqlx::query_as("SELECT id FROM agents WHERE id = ?")
            .bind(agent_id.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(backend)?;
        if agent.is_none() {
            return Err(RepoError::AgentNotFound(agent_id.clone()));
        }
        let skills = dedup_agent_skills(skills);
        for skill in &skills {
            let found: Option<(String,)> = sqlx::query_as("SELECT id FROM skills WHERE id = ?")
                .bind(skill.skill_id.as_str())
                .fetch_optional(&mut *tx)
                .await
                .map_err(backend)?;
            if found.is_none() {
                return Err(RepoError::SkillNotFound(skill.skill_id.clone()));
            }
        }
        sqlx::query("DELETE FROM agent_skills WHERE agent_id = ?")
            .bind(agent_id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        for (position, skill) in skills.iter().enumerate() {
            sqlx::query(
                "INSERT INTO agent_skills (agent_id, skill_id, position, enabled) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(agent_id.as_str())
            .bind(skill.skill_id.as_str())
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .bind(skill.enabled)
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        }
        tx.commit().await.map_err(backend)
    }

    async fn skill_users(&self, skill_id: &SkillId) -> RepoResult<Vec<AgentId>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT agent_id FROM agent_skills WHERE skill_id = ? ORDER BY agent_id",
        )
        .bind(skill_id.as_str())
        .fetch_all(self.pool())
        .await
        .map_err(backend)?;
        Ok(rows
            .into_iter()
            .map(|(id,)| AgentId::from_raw(id))
            .collect())
    }
}
