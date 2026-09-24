//! Quadro Kanban da equipe (`docs/13`, Fase 06): a memória compartilhada do trabalho.
//!
//! Toda regra mora aqui — `claim` atômico, WIP, bloqueio com motivo, dependências sem
//! ciclo, histórico imutável, automações e gate de revisão. A CLI, o MCP e a UI chamam o
//! mesmo [`BoardService`]; nenhuma fachada decide nada sozinha.

mod automation;
mod model;
mod render;
mod repo;
mod rules;
mod service;

#[cfg(test)]
mod tests;

pub use automation::{
    defaults as default_automations, from_toml as automations_from_toml,
    to_toml as automations_to_toml, validate as validate_automation, Action, Automation, Trigger,
};
pub use model::{
    normalize_label, Activity, Actor, Board, Card, CardLink, CardPriority, ChecklistItem, Column,
    ColumnKind, Comment, LinkKind, BODY_MAX, TITLE_MAX,
};
pub use render::{ago, render_board, render_card, render_cards, short_id, BOARD_COLUMN_LINES};
pub use repo::{BoardRepository, CardQuery, CardWrite, WriteGuard};
pub use rules::{
    check_dependency, check_transition, check_wip, column_by_slug, default_columns,
    dependency_path, first_of_kind, is_open, valid_slug, Wip,
};
pub use service::{
    ensure_board, AgentTag, BoardEvent, BoardObserver, BoardService, BoardStore, BoardView,
    CardDetail, CardFilter, CardPatch, CardRef, CardView, ColumnDraft, Gate, GateFailure,
    GateFuture, Moved, NewCard, NoBoardObserver, NoGate, AUTOMATION_MAX_DEPTH,
    CARDS_PER_AGENT_PER_HOUR,
};

use crate::ids::CardId;
use crate::repo::RepoError;

/// Erros do quadro. A mensagem é a da tabela de `docs/13` ("Erros que a CLI precisa
/// devolver bem"): é ela que decide se o agente se recupera sozinho ou trava.
#[derive(Debug, thiserror::Error)]
pub enum BoardError {
    #[error("{card} já foi pego por {by} há {secs}s.")]
    AlreadyClaimed { card: CardId, by: String, secs: u64 },
    #[error("{column} está no limite ({count}/{limit}). Conclua ou devolva um cartão antes de pegar outro.")]
    WipExceeded {
        column: String,
        count: u32,
        limit: u32,
    },
    #[error("{column} aceita {limit} cartão(ões) por agente e {who} já tem {count}. Conclua ou devolva um antes de pegar outro.")]
    WipPerAgent {
        column: String,
        who: String,
        count: u32,
        limit: u32,
    },
    #[error("Bloquear exige --reason. Diga o que falta e quem pode destravar.")]
    ReasonRequired,
    #[error("Rejeitar exige --reason. Diga o que precisa mudar para passar.")]
    RejectReasonRequired,
    #[error("Coluna '{slug}' não existe. Colunas: {}.", available.join(", "))]
    UnknownColumn {
        slug: String,
        available: Vec<String>,
    },
    #[error("Isso criaria um ciclo: {}.", .0.iter().map(CardId::as_str).collect::<Vec<_>>().join(" → "))]
    DependencyCycle(Vec<CardId>),
    #[error("Você fez este cartão, então não pode aprová-lo.")]
    SelfApproval { suggest: Vec<String> },
    #[error("O comando '{command}' falhou ({}). A saída está anexada ao cartão.", exit.map_or_else(|| "sem código de saída".to_owned(), |c| format!("exit {c}")))]
    GateFailed { command: String, exit: Option<i32> },
    #[error("{column} exige aprovação para entrar.")]
    ApprovalRequired { column: String },
    #[error("O cartão {0} não existe nesta equipe.")]
    UnknownCard(String),
    #[error("Não há agente @{0} nesta equipe.")]
    UnknownAgent(String),
    #[error("O cartão {0} está arquivado.")]
    Archived(CardId),
    #[error(
        "O cartão {card} mudou enquanto você mexia ({attempts} tentativas). Leia de novo e repita."
    )]
    Conflict { card: CardId, attempts: u32 },
    #[error("{0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl BoardError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AlreadyClaimed { .. } => "already_claimed",
            Self::WipExceeded { .. } | Self::WipPerAgent { .. } => "wip_exceeded",
            Self::ReasonRequired | Self::RejectReasonRequired => "reason_required",
            Self::UnknownColumn { .. } => "unknown_column",
            Self::DependencyCycle(_) => "dependency_cycle",
            Self::SelfApproval { .. } => "self_approval",
            Self::GateFailed { .. } => "gate_failed",
            Self::ApprovalRequired { .. } => "approval_required",
            Self::UnknownCard(_) => "unknown_card",
            Self::UnknownAgent(_) => "unknown_agent",
            Self::Archived(_) => "card_archived",
            Self::Conflict { .. } => "conflict",
            Self::InvalidRequest(_) => "invalid_request",
            Self::Repo(_) => "internal",
        }
    }

    /// O próximo passo, pronto para a CLI imprimir.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::AlreadyClaimed { .. } => {
                Some("Use 'aisense task next' para o próximo.".into())
            }
            Self::WipExceeded { .. } | Self::WipPerAgent { .. } => Some(
                "Veja o que está em andamento com: aisense task list --mine".into(),
            ),
            Self::SelfApproval { suggest } if !suggest.is_empty() => Some(format!(
                "Peça a {}.",
                suggest
                    .iter()
                    .map(|h| format!("@{h}"))
                    .collect::<Vec<_>>()
                    .join(" ou ")
            )),
            Self::SelfApproval { .. } => Some("Peça a outro agente ou a você (humano).".into()),
            Self::GateFailed { .. } => Some(
                "Leia a saída com: aisense task show <id> — corrija e mande para revisão de novo."
                    .into(),
            ),
            Self::ApprovalRequired { .. } => Some(
                "Mande para revisão (aisense task move <id> review); quem revisa usa aisense task approve <id>."
                    .into(),
            ),
            Self::UnknownCard(_) => Some("Veja os cartões com: aisense board".into()),
            Self::UnknownAgent(_) => Some("Veja quem está na equipe com: aisense agents".into()),
            _ => None,
        }
    }
}

pub type BoardResult<T> = Result<T, BoardError>;
