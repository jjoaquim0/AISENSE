//! Erros de validação do domínio.

use crate::agent::HandleProblem;

/// Uma entrada foi recusada antes de chegar ao banco.
///
/// As mensagens são em inglês (código); a interface traduz pelo `code` da
/// fronteira do Tauri.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("invalid handle {handle:?}: {problem}")]
    InvalidHandle {
        handle: String,
        problem: HandleProblem,
    },
    #[error("handle @{0} is reserved")]
    ReservedHandle(String),
    #[error("handle @{0} is already taken in this team")]
    DuplicateHandle(String),
    #[error("{field} must not be empty")]
    Empty { field: &'static str },
    #[error("{field} must be at most {max} characters")]
    TooLong { field: &'static str, max: usize },
    #[error("invalid environment variable name {0:?}")]
    InvalidEnvKey(String),
    #[error("environment variable {0} is managed by AISENSE and cannot be overridden")]
    ReservedEnvKey(String),
    #[error("the new order must list every agent of the team exactly once")]
    InvalidOrder,
}

impl ValidationError {
    /// Código estável para a interface decidir a mensagem.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidHandle { .. } => "invalid_handle",
            Self::ReservedHandle(_) => "reserved_handle",
            Self::DuplicateHandle(_) => "duplicate_handle",
            Self::Empty { .. } => "empty_field",
            Self::TooLong { .. } => "field_too_long",
            Self::InvalidEnvKey(_) => "invalid_env_key",
            Self::ReservedEnvKey(_) => "reserved_env_key",
            Self::InvalidOrder => "invalid_order",
        }
    }
}

/// Recorta espaços e exige conteúdo, com limite em caracteres (não bytes).
pub(crate) fn required_text(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<String, ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if trimmed.chars().count() > max {
        return Err(ValidationError::TooLong { field, max });
    }
    Ok(trimmed.to_owned())
}

/// Texto opcional: vazio vira `None`.
pub(crate) fn optional_text(
    field: &'static str,
    value: Option<&str>,
    max: usize,
) -> Result<Option<String>, ValidationError> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => required_text(field, v, max).map(Some),
    }
}
