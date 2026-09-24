//! Tipos do auto-update (F09-03). Quem baixa, confere a assinatura e instala é o
//! `tauri-plugin-updater`, no app; aqui só o que atravessa para a interface (R5).

use serde::Serialize;
use ts_rs::TS;

/// Uma versão nova disponível no canal estável.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Notas do release (Markdown), se o manifesto trouxer.
    pub notes: Option<String>,
    /// Data de publicação, como o manifesto informa.
    pub date: Option<String>,
}

/// Payload do evento `update:progress` durante o download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct UpdateProgress {
    #[ts(type = "number")]
    pub downloaded: u64,
    /// Tamanho total, quando o servidor informa.
    #[ts(type = "number | null")]
    pub total: Option<u64>,
}
