//! Skill já validada, como o resto do app a enxerga.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Skill {
    /// Slug único, `^[a-z][a-z0-9-]*$` (ex.: `revisor-rigoroso`).
    pub name: String,
    /// Uma frase dizendo **quando** usar — é o que a IA lê para decidir se aplica.
    pub description: String,
    pub version: String,
    /// `adapter_id`s compatíveis; vazio = todos.
    pub targets: Vec<String>,
    pub inject: SkillInject,
    /// Ordem de injeção, 0–100; menor vem primeiro.
    pub priority: u8,
    pub env: BTreeMap<String, String>,
    /// O Markdown depois do frontmatter.
    pub body: String,
    pub source: SkillSource,
}

impl Skill {
    /// Compatível com o runtime? `targets` vazio aceita qualquer um.
    pub fn supports(&self, adapter_id: &str) -> bool {
        self.targets.is_empty() || self.targets.iter().any(|t| t == adapter_id)
    }
}

/// Como a skill chega ao agente (`docs/06`, campo `inject`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum SkillInject {
    /// Entra no prompt inicial (`BOOT.md`).
    #[default]
    Bootstrap,
    /// Só o arquivo é materializado; a IA lê sob demanda.
    Reference,
    /// Exposta como ferramenta pelo servidor MCP.
    Mcp,
}

/// De onde a skill veio — a UI mostra "embutida" ou a pasta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum SkillSource {
    Builtin,
    /// `dir` é a pasta da skill (a que contém o `SKILL.md`), com os arquivos de apoio.
    User {
        dir: String,
    },
}

/// Um `SKILL.md` que não carregou. Nunca derruba o app: vira aviso na UI com o caminho
/// e, quando dá para saber, a linha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SkillProblem {
    pub path: String,
    pub line: Option<u32>,
    pub message: String,
}

impl std::fmt::Display for SkillProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
        }
        write!(f, ": {}", self.message)
    }
}
