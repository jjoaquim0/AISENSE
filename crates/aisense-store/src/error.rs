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
    /// A migração falhou e o banco voltou a ser o backup: nada se perdeu, mas esta
    /// versão do app não consegue usá-lo (F09-04).
    #[error("database migration failed and the database was restored from {backup}: {source}")]
    MigrationRolledBack {
        backup: PathBuf,
        #[source]
        source: sqlx::migrate::MigrateError,
    },
    /// A migração falhou e nem o backup pôde voltar: o backup é a cópia boa.
    #[error(
        "database migration failed ({migration}) and restoring {backup} also failed: {source}"
    )]
    RestoreFailed {
        backup: PathBuf,
        migration: sqlx::migrate::MigrateError,
        #[source]
        source: std::io::Error,
    },
    #[error("database migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}
