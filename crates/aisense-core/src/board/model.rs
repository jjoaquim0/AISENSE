//! Entidades do quadro (`docs/13`, "Modelo"): um quadro por equipe, colunas com tipo
//! semântico, cartões com checklist, labels, links, dependências, comentários e histórico.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::automation::Automation;
use crate::ids::{ActivityId, AgentId, BoardId, CardId, ColumnId, CommentId, TeamId};
use crate::time::Millis;

/// O que a coluna significa, independente do nome que a equipe deu a ela. Automações e
/// agentes decidem por aqui ("de onde puxo trabalho?", "o que é concluído?").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum ColumnKind {
    /// Ideias, ainda sem prioridade.
    Intake,
    /// Pronto para alguém pegar: é daqui que `aisense task next` puxa.
    Ready,
    /// Em execução.
    Active,
    /// Esperando algo de fora; entrar exige motivo.
    Blocked,
    /// Feito, aguardando outro olhar.
    Review,
    /// Concluído.
    Terminal,
}

impl ColumnKind {
    pub const ALL: [ColumnKind; 6] = [
        Self::Intake,
        Self::Ready,
        Self::Active,
        Self::Blocked,
        Self::Review,
        Self::Terminal,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intake => "intake",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Blocked => "blocked",
            Self::Review => "review",
            Self::Terminal => "terminal",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == raw)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Column {
    pub id: ColumnId,
    pub board_id: BoardId,
    /// Identificador estável que a CLI usa (`aisense task move tsk_x doing`).
    pub slug: String,
    pub name: String,
    pub kind: ColumnKind,
    /// Cartões na coluna, somando todos. `None` = sem limite.
    pub wip_limit: Option<u32>,
    /// Cartões por responsável na coluna ("1 por agente" em Fazendo).
    pub wip_per_agent: Option<u32>,
    pub position: u32,
    /// Gate de revisão (`docs/13`): só entra com aprovação.
    pub requires_approval: bool,
    /// Quem fez não aprova.
    pub approver_must_differ: bool,
    /// Comandos do `aisense.toml` que precisam passar para entrar.
    pub requires_commands: Vec<String>,
}

impl Column {
    /// Gate de revisão ligado de algum jeito.
    pub fn gated(&self) -> bool {
        self.requires_approval || !self.requires_commands.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Board {
    pub id: BoardId,
    pub team_id: TeamId,
    pub automations: Vec<Automation>,
    pub created_at: Millis,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum CardPriority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

impl CardPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }

    /// Aceita também os nomes em português da CLI (`--priority alta`).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "low" | "baixa" => Some(Self::Low),
            "normal" | "media" | "média" => Some(Self::Normal),
            "high" | "alta" => Some(Self::High),
            "urgent" | "urgente" => Some(Self::Urgent),
            _ => None,
        }
    }

    /// Como `aisense board` escreve.
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "baixa",
            Self::Normal => "média",
            Self::High => "alta",
            Self::Urgent => "urgente",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct ChecklistItem {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum LinkKind {
    Pr,
    Commit,
    File,
    Url,
}

impl LinkKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pr" => Some(Self::Pr),
            "commit" => Some(Self::Commit),
            "file" => Some(Self::File),
            "url" => Some(Self::Url),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct CardLink {
    pub kind: LinkKind,
    pub target: String,
}

/// Quem fez: um agente, você ou o próprio AISENSE (automação, gate).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum Actor {
    Human,
    #[serde(rename_all = "camelCase")]
    Agent {
        agent_id: AgentId,
    },
    System,
}

impl Actor {
    pub fn agent(&self) -> Option<&AgentId> {
        match self {
            Self::Agent { agent_id } => Some(agent_id),
            _ => None,
        }
    }

    /// `(author_kind, author)` como vão para o banco.
    pub fn to_columns(&self) -> (&'static str, Option<&str>) {
        match self {
            Self::Human => ("human", None),
            Self::Agent { agent_id } => ("agent", Some(agent_id.as_str())),
            Self::System => ("system", None),
        }
    }

    /// Volta do banco. Agente removido (`author` NULL) vira humano? Não: vira sistema, que
    /// é o mais honesto — sabemos que não foi você.
    pub fn from_columns(kind: &str, author: Option<String>) -> Option<Self> {
        match (kind, author) {
            ("human", _) => Some(Self::Human),
            ("agent", Some(id)) => Some(Self::Agent {
                agent_id: AgentId::from_raw(id),
            }),
            ("agent", None) | ("system", _) => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Card {
    pub id: CardId,
    pub team_id: TeamId,
    pub column_id: ColumnId,
    pub title: String,
    /// Markdown.
    pub body: String,
    pub assignee: Option<AgentId>,
    /// `None` = você ou o sistema.
    pub created_by: Option<AgentId>,
    pub parent_id: Option<CardId>,
    pub position: i64,
    pub priority: CardPriority,
    pub labels: Vec<String>,
    pub checklist: Vec<ChecklistItem>,
    pub links: Vec<CardLink>,
    pub block_reason: Option<String>,
    /// Trava otimista: toda gravação sobe um. `claim` só vale sobre a versão lida.
    pub version: i64,
    pub archived_at: Option<Millis>,
    pub approved_by: Option<AgentId>,
    pub approved_at: Option<Millis>,
    /// Desde quando está na coluna atual (`card_stale`, "há 18min").
    pub column_since: Millis,
    pub created_at: Millis,
    pub updated_at: Millis,
}

impl Card {
    /// Itens marcados e total.
    pub fn checklist_progress(&self) -> (usize, usize) {
        (
            self.checklist.iter().filter(|i| i.done).count(),
            self.checklist.len(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Comment {
    pub id: CommentId,
    pub card_id: CardId,
    pub author: Actor,
    pub body: String,
    pub created_at: Millis,
}

/// Uma linha do histórico imutável do cartão.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Activity {
    pub id: ActivityId,
    pub card_id: CardId,
    pub actor: Actor,
    /// `created`, `moved`, `assigned`, `updated`, `checked`, `commented`, `linked`,
    /// `blocked`, `dependency`, `approved`, `rejected`, `gate_failed`, `archived`.
    pub action: String,
    /// O diff: `{"campo": [antes, depois]}` ou o detalhe da ação.
    #[ts(type = "Record<string, unknown>")]
    pub detail: serde_json::Value,
    pub created_at: Millis,
}

/// Rótulo normalizado: minúsculo, sem espaço nas pontas, até 32 caracteres.
pub fn normalize_label(raw: &str) -> Option<String> {
    let label = raw.trim().to_lowercase();
    if label.is_empty() || label.chars().count() > 32 || label.contains(',') {
        None
    } else {
        Some(label)
    }
}

/// Limite do título (uma linha do `aisense board`).
pub const TITLE_MAX: usize = 200;
/// Limite do corpo e de comentários.
pub const BODY_MAX: usize = 64 * 1024;
