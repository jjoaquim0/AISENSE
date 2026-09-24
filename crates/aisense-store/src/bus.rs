//! `BusRepository` sobre SQLite (tabelas `channels`, `messages`, `deliveries`; F05-02).

use aisense_core::bus::{
    BusRepository, Channel, Delivery, DeliveryState, InboxItem, InboxQuery, Message, MessageKind,
    MessageMeta, Sender, Target,
};
use aisense_core::repo::{RepoError, RepoResult};
use aisense_core::{AgentId, ChannelId, MessageId, Millis, TeamId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::convert::{backend, corrupt, is_foreign_key_violation, json, text_enum, to_json};
use crate::Store;

const MESSAGE_COLUMNS: &str = "m.id, m.team_id, m.kind, m.from_kind, m.from_agent, m.to_agent, \
     m.to_channel, m.broadcast, m.to_human, m.reply_to, m.subject, m.body, m.meta, m.created_at";
const DELIVERY_COLUMNS: &str =
    "d.message_id, d.agent_id, d.state, d.delivered_at, d.read_at, d.attempts, d.error";

fn message_from_row(row: &SqliteRow) -> RepoResult<Message> {
    let text = |col: &str| row.try_get::<String, _>(col).map_err(backend);
    let opt = |col: &str| row.try_get::<Option<String>, _>(col).map_err(backend);
    let flag = |col: &str| row.try_get::<bool, _>(col).map_err(backend);
    let from = match text("from_kind")?.as_str() {
        "agent" => Sender::Agent {
            agent_id: AgentId::from_raw(
                opt("from_agent")?.ok_or_else(|| corrupt("messages", "from_agent", "NULL"))?,
            ),
        },
        "human" => Sender::Human,
        "system" => Sender::System,
        other => return Err(corrupt("messages", "from_kind", other)),
    };
    let to = if let Some(agent) = opt("to_agent")? {
        Target::Agent {
            agent_id: AgentId::from_raw(agent),
        }
    } else if let Some(channel) = opt("to_channel")? {
        Target::Channel {
            channel_id: ChannelId::from_raw(channel),
        }
    } else if flag("broadcast")? {
        Target::Team
    } else if flag("to_human")? {
        Target::Human
    } else {
        return Err(corrupt("messages", "to_*", "no target"));
    };
    let kind = text("kind")?;
    Ok(Message {
        id: MessageId::from_raw(text("id")?),
        team_id: TeamId::from_raw(text("team_id")?),
        kind: text_enum("messages", "kind", &kind, MessageKind::parse)?,
        from,
        to,
        reply_to: opt("reply_to")?.map(MessageId::from_raw),
        subject: opt("subject")?,
        body: text("body")?,
        meta: json::<MessageMeta>("messages", "meta", &text("meta")?)?,
        created_at: row.try_get("created_at").map_err(backend)?,
    })
}

fn delivery_from_row(row: &SqliteRow) -> RepoResult<Delivery> {
    let state = row.try_get::<String, _>("state").map_err(backend)?;
    Ok(Delivery {
        message_id: MessageId::from_raw(row.try_get::<String, _>("message_id").map_err(backend)?),
        agent_id: AgentId::from_raw(row.try_get::<String, _>("agent_id").map_err(backend)?),
        state: text_enum("deliveries", "state", &state, DeliveryState::parse)?,
        delivered_at: row.try_get("delivered_at").map_err(backend)?,
        read_at: row.try_get("read_at").map_err(backend)?,
        attempts: row
            .try_get::<i64, _>("attempts")
            .map_err(backend)?
            .try_into()
            .unwrap_or(0),
        error: row.try_get("error").map_err(backend)?,
    })
}

fn channel_from_row(row: &SqliteRow) -> RepoResult<Channel> {
    let text = |col: &str| row.try_get::<String, _>(col).map_err(backend);
    Ok(Channel {
        id: ChannelId::from_raw(text("id")?),
        team_id: TeamId::from_raw(text("team_id")?),
        slug: text("slug")?,
        topic: text("topic")?,
        created_at: row.try_get("created_at").map_err(backend)?,
    })
}

impl BusRepository for Store {
    async fn channel_by_slug(&self, team_id: &TeamId, slug: &str) -> RepoResult<Option<Channel>> {
        sqlx::query("SELECT * FROM channels WHERE team_id = ? AND slug = ?")
            .bind(team_id.as_str())
            .bind(slug)
            .fetch_optional(self.pool())
            .await
            .map_err(backend)?
            .as_ref()
            .map(channel_from_row)
            .transpose()
    }

    async fn create_channel(&self, channel: &Channel) -> RepoResult<()> {
        sqlx::query(
            "INSERT INTO channels (id, team_id, slug, topic, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(channel.id.as_str())
        .bind(channel.team_id.as_str())
        .bind(&channel.slug)
        .bind(&channel.topic)
        .bind(channel.created_at)
        .execute(self.pool())
        .await
        .map_err(|e| {
            if e.as_database_error()
                .is_some_and(|db| db.is_unique_violation())
            {
                RepoError::AlreadyExists(format!("#{}", channel.slug))
            } else if is_foreign_key_violation(&e) {
                RepoError::TeamNotFound(channel.team_id.clone())
            } else {
                backend(e)
            }
        })?;
        Ok(())
    }

    async fn list_channels(&self, team_id: &TeamId) -> RepoResult<Vec<Channel>> {
        sqlx::query("SELECT * FROM channels WHERE team_id = ? ORDER BY slug")
            .bind(team_id.as_str())
            .fetch_all(self.pool())
            .await
            .map_err(backend)?
            .iter()
            .map(channel_from_row)
            .collect()
    }

    async fn insert_message(&self, message: &Message, deliveries: &[Delivery]) -> RepoResult<()> {
        let (from_kind, from_agent) = match &message.from {
            Sender::Agent { agent_id } => ("agent", Some(agent_id.as_str())),
            Sender::Human => ("human", None),
            Sender::System => ("system", None),
        };
        let (to_agent, to_channel, broadcast, to_human) = match &message.to {
            Target::Agent { agent_id } => (Some(agent_id.as_str()), None, false, false),
            Target::Channel { channel_id } => (None, Some(channel_id.as_str()), false, false),
            Target::Team => (None, None, true, false),
            Target::Human => (None, None, false, true),
        };
        let meta = to_json(&message.meta)?;
        let mut tx = self.pool().begin().await.map_err(backend)?;
        sqlx::query(
            "INSERT INTO messages (id, team_id, kind, from_kind, from_agent, to_agent, to_channel, \
             broadcast, to_human, reply_to, subject, body, meta, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(message.id.as_str())
        .bind(message.team_id.as_str())
        .bind(message.kind.as_str())
        .bind(from_kind)
        .bind(from_agent)
        .bind(to_agent)
        .bind(to_channel)
        .bind(broadcast)
        .bind(to_human)
        .bind(message.reply_to.as_ref().map(MessageId::as_str))
        .bind(message.subject.as_deref())
        .bind(&message.body)
        .bind(&meta)
        .bind(message.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if is_foreign_key_violation(&e) {
                RepoError::TeamNotFound(message.team_id.clone())
            } else if e.as_database_error().is_some_and(|db| db.is_unique_violation()) {
                RepoError::AlreadyExists(message.id.to_string())
            } else {
                backend(e)
            }
        })?;
        for d in deliveries {
            sqlx::query(
                "INSERT INTO deliveries (message_id, agent_id, state, delivered_at, read_at, \
                 attempts, error) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(d.message_id.as_str())
            .bind(d.agent_id.as_str())
            .bind(d.state.as_str())
            .bind(d.delivered_at)
            .bind(d.read_at)
            .bind(i64::from(d.attempts))
            .bind(d.error.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                if is_foreign_key_violation(&e) {
                    RepoError::AgentNotFound(d.agent_id.clone())
                } else {
                    backend(e)
                }
            })?;
        }
        tx.commit().await.map_err(backend)
    }

    async fn get_message(&self, id: &MessageId) -> RepoResult<Option<Message>> {
        sqlx::query(&format!(
            "SELECT {MESSAGE_COLUMNS} FROM messages m WHERE m.id = ?"
        ))
        .bind(id.as_str())
        .fetch_optional(self.pool())
        .await
        .map_err(backend)?
        .as_ref()
        .map(message_from_row)
        .transpose()
    }

    async fn deliveries_of(&self, message_id: &MessageId) -> RepoResult<Vec<Delivery>> {
        sqlx::query(&format!(
            "SELECT {DELIVERY_COLUMNS} FROM deliveries d WHERE d.message_id = ? ORDER BY d.agent_id"
        ))
        .bind(message_id.as_str())
        .fetch_all(self.pool())
        .await
        .map_err(backend)?
        .iter()
        .map(delivery_from_row)
        .collect()
    }

    async fn inbox(&self, agent_id: &AgentId, query: &InboxQuery) -> RepoResult<Vec<InboxItem>> {
        let unread = if query.unread_only {
            "AND d.state IN ('pending', 'delivered')"
        } else {
            ""
        };
        let rows = sqlx::query(&format!(
            "SELECT {MESSAGE_COLUMNS}, {DELIVERY_COLUMNS} FROM deliveries d \
             JOIN messages m ON m.id = d.message_id \
             WHERE d.agent_id = ? AND (? IS NULL OR d.message_id > ?) {unread} \
             ORDER BY d.message_id LIMIT ?"
        ))
        .bind(agent_id.as_str())
        .bind(query.after.as_ref().map(MessageId::as_str))
        .bind(query.after.as_ref().map(MessageId::as_str))
        .bind(i64::from(query.limit))
        .fetch_all(self.pool())
        .await
        .map_err(backend)?;
        rows.iter()
            .map(|row| {
                Ok(InboxItem {
                    message: message_from_row(row)?,
                    delivery: delivery_from_row(row)?,
                })
            })
            .collect()
    }

    async fn mark_read(
        &self,
        agent_id: &AgentId,
        ids: &[MessageId],
        now: Millis,
    ) -> RepoResult<u32> {
        let mut tx = self.pool().begin().await.map_err(backend)?;
        let mut changed = 0u64;
        for id in ids {
            changed += sqlx::query(
                "UPDATE deliveries SET state = 'read', read_at = ? \
                 WHERE message_id = ? AND agent_id = ? AND state IN ('pending', 'delivered')",
            )
            .bind(now)
            .bind(id.as_str())
            .bind(agent_id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(backend)?
            .rows_affected();
        }
        tx.commit().await.map_err(backend)?;
        Ok(u32::try_from(changed).unwrap_or(u32::MAX))
    }

    async fn update_delivery(&self, d: &Delivery) -> RepoResult<()> {
        let done = sqlx::query(
            "UPDATE deliveries SET state = ?, delivered_at = ?, read_at = ?, attempts = ?, \
             error = ? WHERE message_id = ? AND agent_id = ?",
        )
        .bind(d.state.as_str())
        .bind(d.delivered_at)
        .bind(d.read_at)
        .bind(i64::from(d.attempts))
        .bind(d.error.as_deref())
        .bind(d.message_id.as_str())
        .bind(d.agent_id.as_str())
        .execute(self.pool())
        .await
        .map_err(backend)?;
        if done.rows_affected() == 0 {
            return Err(RepoError::Corrupt(format!(
                "no delivery of {} to {}",
                d.message_id, d.agent_id
            )));
        }
        Ok(())
    }

    async fn timeline(
        &self,
        team_id: &TeamId,
        before: Option<&MessageId>,
        limit: u32,
    ) -> RepoResult<Vec<Message>> {
        sqlx::query(&format!(
            "SELECT {MESSAGE_COLUMNS} FROM messages m \
             WHERE m.team_id = ? AND (? IS NULL OR m.id < ?) ORDER BY m.id DESC LIMIT ?"
        ))
        .bind(team_id.as_str())
        .bind(before.map(MessageId::as_str))
        .bind(before.map(MessageId::as_str))
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await
        .map_err(backend)?
        .iter()
        .map(message_from_row)
        .collect()
    }

    async fn replies_to(&self, id: &MessageId) -> RepoResult<Vec<Message>> {
        sqlx::query(&format!(
            "SELECT {MESSAGE_COLUMNS} FROM messages m WHERE m.reply_to = ? ORDER BY m.id"
        ))
        .bind(id.as_str())
        .fetch_all(self.pool())
        .await
        .map_err(backend)?
        .iter()
        .map(message_from_row)
        .collect()
    }

    async fn unread_counts(&self, team_id: &TeamId) -> RepoResult<Vec<(AgentId, u32)>> {
        let rows = sqlx::query(
            "SELECT d.agent_id, COUNT(*) AS n FROM deliveries d \
             JOIN agents a ON a.id = d.agent_id \
             WHERE a.team_id = ? AND d.state IN ('pending', 'delivered') \
             GROUP BY d.agent_id ORDER BY d.agent_id",
        )
        .bind(team_id.as_str())
        .fetch_all(self.pool())
        .await
        .map_err(backend)?;
        rows.iter()
            .map(|row| {
                let id: String = row.try_get("agent_id").map_err(backend)?;
                let n: i64 = row.try_get("n").map_err(backend)?;
                Ok((AgentId::from_raw(id), u32::try_from(n).unwrap_or(u32::MAX)))
            })
            .collect()
    }

    async fn prune_messages(&self, before: Millis) -> RepoResult<u64> {
        Ok(sqlx::query("DELETE FROM messages WHERE created_at < ?")
            .bind(before)
            .execute(self.pool())
            .await
            .map_err(backend)?
            .rows_affected())
    }
}
