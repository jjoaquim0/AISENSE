//! Adaptador de runtime já validado, como o resto do app o enxerga (`docs/05`).
//!
//! O formato em disco (snake_case, campos opcionais) mora em `file.rs`; aqui fica a
//! forma normalizada, com padrões aplicados, que vai para o supervisor e para a UI.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Adapter {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    /// Pode conter `$SHELL`, que o supervisor resolve para o shell do sistema.
    pub command: String,
    pub args: Vec<String>,
    pub detect: Option<DetectSpec>,
    pub install_hint: Option<String>,
    pub capabilities: Capabilities,
    pub state: StateRules,
    pub inject: InjectRules,
    pub skills: Option<SkillsTarget>,
    pub env: BTreeMap<String, String>,
    pub source: AdapterSource,
}

/// Comando que responde rápido se o runtime está instalado (ex.: `claude --version`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct DetectSpec {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Capabilities {
    pub mcp: bool,
    pub system_prompt_flag: Option<String>,
    pub hooks: bool,
    pub model_flag: Option<String>,
    pub cwd_is_project: bool,
    pub resume_flag: Option<String>,
}

/// Heurística do detector de estado. Os regex já foram compilados uma vez na
/// carga, então quem usa pode confiar que são válidos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct StateRules {
    pub idle_regex: Option<String>,
    pub busy_regex: Option<String>,
    pub awaiting_regex: Option<String>,
    pub quiet_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum InjectMode {
    Stdin,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct InjectRules {
    pub mode: InjectMode,
    pub submit: String,
    pub prefix: String,
    pub max_chars: u32,
    /// Se o `BOOT.md` pode entrar pelo terminal (F04-06). `false` num shell puro: digitar
    /// "leia o BOOT.md" ali viraria um comando inexistente.
    pub boot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SkillsTarget {
    pub dir: String,
    pub format: Option<String>,
    pub settings_file: Option<String>,
}

/// De onde o adaptador veio — a UI mostra "embutido" ou o caminho do arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum AdapterSource {
    Builtin,
    User { path: String },
}

/// Um arquivo de adaptador que não carregou. Nunca derruba o app: vira aviso na UI
/// com o caminho e, quando dá para saber, a linha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AdapterProblem {
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
}

impl std::fmt::Display for AdapterProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
            if let Some(column) = self.column {
                write!(f, ":{column}")?;
            }
        }
        write!(f, ": {}", self.message)
    }
}
