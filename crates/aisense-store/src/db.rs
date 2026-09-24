//! Abertura do banco, PRAGMAs e migrações.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::StoreError;

/// Migrações embutidas no binário em tempo de compilação.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Conexão com o banco do AISENSE.
///
/// Clonar é barato: o pool é compartilhado.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Abre (ou cria) o banco em `path`, faz backup se houver migração pendente
    /// num banco que já tem dados, e migra.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent() {
            // Falhar aqui vira um erro de `connect` logo abaixo, com o caminho.
            let _ = std::fs::create_dir_all(dir);
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            // NORMAL é seguro com WAL: um corte de energia perde no máximo a última
            // transação, nunca corrompe o arquivo.
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(|source| StoreError::Open {
                path: path.to_owned(),
                source,
            })?;
        let store = Self { pool };
        store.backup_before_migrating(path).await?;
        store.migrate().await?;
        tracing::info!(path = %path.display(), "database ready");
        Ok(store)
    }

    /// Banco em memória, já migrado. Para testes.
    ///
    /// Uma conexão só, que nunca expira: cada conexão `:memory:` é um banco
    /// diferente, e perder a conexão perde os dados.
    pub async fn open_in_memory() -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// Aplica as migrações pendentes. Idempotente.
    pub async fn migrate(&self) -> Result<(), StoreError> {
        MIGRATOR.run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// Versões já aplicadas neste banco (vazio num banco novo).
    pub async fn applied_versions(&self) -> Result<Vec<i64>, StoreError> {
        let exists: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_optional(&self.pool)
        .await?;
        if exists.is_none() {
            return Ok(Vec::new());
        }
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT version FROM _sqlx_migrations WHERE success = 1 ORDER BY version",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(v,)| v).collect())
    }

    /// Mitigação do risco "migração futura quebrar banco de usuário" (Fase 02):
    /// antes de migrar um banco que já tem dados, copia-o para
    /// `<arquivo>.bak-v<versão atual>`. Banco novo não tem o que proteger.
    async fn backup_before_migrating(&self, path: &Path) -> Result<(), StoreError> {
        let applied = self.applied_versions().await?;
        let Some(current) = applied.last().copied() else {
            return Ok(());
        };
        let pending = MIGRATOR.iter().any(|m| !applied.contains(&m.version));
        if !pending {
            return Ok(());
        }
        let backup = backup_path(path, current);
        // VACUUM INTO falha se o destino existe; um backup antigo da mesma versão
        // é substituído pelo estado atual.
        let _ = std::fs::remove_file(&backup);
        sqlx::query("VACUUM INTO ?")
            .bind(backup.to_string_lossy().into_owned())
            .execute(&self.pool)
            .await
            .map_err(|source| StoreError::Backup {
                path: backup.clone(),
                source,
            })?;
        tracing::info!(backup = %backup.display(), from_version = current, "database backed up before migrating");
        Ok(())
    }
}

fn backup_path(path: &Path, version: i64) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".bak-v{version}"));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    async fn tables(store: &Store) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations' ORDER BY name",
        )
        .fetch_all(store.pool())
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn creates_the_documented_schema_in_memory() {
        let store = Store::open_in_memory().await.unwrap();
        assert_eq!(
            tables(&store).await,
            [
                "agent_skills",
                "agent_tokens",
                "agents",
                "boards",
                "channel_members",
                "channels",
                "columns",
                "deliveries",
                "messages",
                "sessions",
                "skills",
                "task_activity",
                "task_comments",
                "task_dependencies",
                "tasks",
                "teams",
                "workbenches",
            ]
        );
    }

    #[tokio::test]
    async fn migrating_twice_is_a_no_op() {
        let store = Store::open_in_memory().await.unwrap();
        let before = store.applied_versions().await.unwrap();
        store.migrate().await.unwrap();
        store.migrate().await.unwrap();
        assert_eq!(store.applied_versions().await.unwrap(), before);
        assert_eq!(before.len(), MIGRATOR.iter().count());
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced() {
        let store = Store::open_in_memory().await.unwrap();
        let err = sqlx::query(
            "INSERT INTO agents (id, team_id, handle, name, adapter_id, color, created_at, updated_at) \
             VALUES ('agt_1', 'tem_missing', 'backend', 'Backend', 'shell', 'violet', 0, 0)",
        )
        .execute(store.pool())
        .await
        .unwrap_err();
        assert!(err.to_string().contains("FOREIGN KEY"), "{err}");
    }

    #[tokio::test]
    async fn file_database_uses_wal_and_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("aisense.db");

        let store = Store::open(&path).await.unwrap();
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(mode, "wal");
        sqlx::query(
            "INSERT INTO teams (id, name, workdir, created_at, updated_at) VALUES ('tem_1', 'A', '/tmp', 0, 0)",
        )
        .execute(store.pool())
        .await
        .unwrap();
        store.close().await;

        let again = Store::open(&path).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM teams")
            .fetch_one(again.pool())
            .await
            .unwrap();
        assert_eq!(count, 1);
        let latest = MIGRATOR.iter().map(|m| m.version).max().unwrap();
        assert!(
            !backup_path(&path, latest).exists(),
            "an up-to-date database is not backed up"
        );
    }

    #[tokio::test]
    async fn backs_up_before_applying_a_pending_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("aisense.db");

        // Simula um usuário que ainda está na versão 1: aplica só a primeira migração.
        {
            let options = SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .connect_with(options)
                .await
                .unwrap();
            let only_first = Migrator {
                migrations: std::borrow::Cow::Owned(vec![MIGRATOR.iter().next().unwrap().clone()]),
                ..Migrator::DEFAULT
            };
            only_first.run(&pool).await.unwrap();
            pool.close().await;
        }

        let store = Store::open(&path).await.unwrap();
        // Todas as migrações, não um número fixo: cada migração nova não quebra este teste.
        let all: Vec<i64> = MIGRATOR.iter().map(|m| m.version).collect();
        assert_eq!(store.applied_versions().await.unwrap(), all);
        assert!(backup_path(&path, 1).exists(), "backup of v1 must exist");
    }
}
