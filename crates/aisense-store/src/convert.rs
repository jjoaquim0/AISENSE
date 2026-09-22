//! Tradução linha SQL ⇄ entidade do domínio. Toda coluna `TEXT` com enum ou JSON
//! passa pela validação do core ao ser lida: dado inválido no banco vira
//! `RepoError::Corrupt`, nunca uma entidade inválida em memória.

use aisense_core::repo::RepoError;

pub(crate) fn backend(err: sqlx::Error) -> RepoError {
    RepoError::Backend(Box::new(err))
}

pub(crate) fn corrupt(table: &str, column: &str, value: &str) -> RepoError {
    RepoError::Corrupt(format!("{table}.{column} = {value:?}"))
}

/// Lê um enum textual com o `parse` que o próprio tipo expõe.
pub(crate) fn text_enum<T>(
    table: &str,
    column: &str,
    raw: &str,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T, RepoError> {
    parse(raw).ok_or_else(|| corrupt(table, column, raw))
}

pub(crate) fn json<T: serde::de::DeserializeOwned>(
    table: &str,
    column: &str,
    raw: &str,
) -> Result<T, RepoError> {
    serde_json::from_str(raw).map_err(|_| corrupt(table, column, raw))
}

pub(crate) fn to_json<T: serde::Serialize>(value: &T) -> Result<String, RepoError> {
    serde_json::to_string(value).map_err(|e| RepoError::Backend(Box::new(e)))
}

/// `true` quando o erro é a violação de `UNIQUE (team_id, handle)`.
pub(crate) fn is_handle_conflict(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db| db.is_unique_violation() && db.message().contains("agents.handle"))
}

pub(crate) fn is_primary_key_conflict(err: &sqlx::Error, table: &str) -> bool {
    err.as_database_error()
        .is_some_and(|db| db.is_unique_violation() && db.message().contains(&format!("{table}.id")))
}

pub(crate) fn is_foreign_key_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db| db.is_foreign_key_violation())
}
