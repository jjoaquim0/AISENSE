//! Erro serializável para a interface.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// O que a UI recebe quando um comando falha.
///
/// `code` é para a interface decidir o que fazer; `message` é o que aconteceu;
/// `hint` é o próximo passo. Erro que só diz "falhou" faz o usuário abrir um
/// chamado — erro que diz o que fazer resolve sozinho (`docs/10`).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub hint: Option<String>,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, hint: Option<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            hint,
        }
    }
}
