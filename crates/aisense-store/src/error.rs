use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not open database at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: sqlx::Error,
    },
    #[error("could not back up database to {path}: {source}")]
    Backup {
        path: PathBuf,
        #[source]
        source: sqlx::Error,
    },
    #[error("database migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}
