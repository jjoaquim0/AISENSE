//! Informação de identificação do aplicativo.
//!
//! Mora no core (e não no crate do Tauri) para que `pnpm gen:types` funcione sem
//! precisar compilar a janela — o que exige GTK/WebKit no Linux.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AppInfo {
    /// Versão do aplicativo.
    pub version: String,
    /// Sistema operacional em que o core está rodando.
    pub platform: String,
    /// `true` em build de desenvolvimento.
    pub debug: bool,
}

impl AppInfo {
    pub fn current() -> Self {
        Self {
            version: crate::VERSION.to_string(),
            platform: std::env::consts::OS.to_string(),
            debug: cfg!(debug_assertions),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_non_empty_version_and_platform() {
        let info = AppInfo::current();
        assert!(!info.version.is_empty());
        assert!(!info.platform.is_empty());
    }
}
