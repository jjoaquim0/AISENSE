//! Regras do quadro como funções puras (`docs/13`, "Regras de integridade"). O serviço
//! consulta o banco e chama estas; a UI e a CLI nunca decidem sozinhas.

use std::collections::{BTreeMap, BTreeSet};

use super::model::{Card, Column, ColumnKind};
use super::BoardError;
use crate::ids::{AgentId, BoardId, CardId, ColumnId};

/// As 6 colunas com que todo quadro nasce (`docs/13`, "Colunas padrão").
pub fn default_columns(board_id: &BoardId) -> Vec<Column> {
    let spec: [(&str, &str, ColumnKind, Option<u32>); 6] = [
        ("backlog", "Backlog", ColumnKind::Intake, None),
        ("todo", "A fazer", ColumnKind::Ready, None),
        ("doing", "Fazendo", ColumnKind::Active, Some(1)),
        ("blocked", "Bloqueada", ColumnKind::Blocked, None),
        ("review", "Revisão", ColumnKind::Review, None),
        ("done", "Feita", ColumnKind::Terminal, None),
    ];
    spec.iter()
        .enumerate()
        .map(|(i, (slug, name, kind, per_agent))| Column {
            id: ColumnId::new(),
            board_id: board_id.clone(),
            slug: (*slug).to_owned(),
            name: (*name).to_owned(),
            kind: *kind,
            wip_limit: None,
            wip_per_agent: *per_agent,
            position: u32::try_from(i).unwrap_or(u32::MAX),
            requires_approval: false,
            approver_must_differ: true,
            requires_commands: Vec::new(),
        })
        .collect()
}

/// Slug de coluna: minúsculas, dígitos e hífen, até 32.
pub fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 32
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
}

pub fn column_by_slug<'a>(columns: &'a [Column], slug: &str) -> Result<&'a Column, BoardError> {
    columns
        .iter()
        .find(|c| c.slug == slug)
        .ok_or_else(|| BoardError::UnknownColumn {
            slug: slug.to_owned(),
            available: columns.iter().map(|c| c.slug.clone()).collect(),
        })
}

pub fn first_of_kind(columns: &[Column], kind: ColumnKind) -> Option<&Column> {
    columns.iter().find(|c| c.kind == kind)
}

/// O que uma mudança de coluna precisa cumprir, fora WIP (que depende do banco) e gate
/// (que depende de quem aprova). `reason` é o motivo dado agora, se houver.
pub fn check_transition(card: &Card, to: &Column, reason: Option<&str>) -> Result<(), BoardError> {
    if card.archived_at.is_some() {
        return Err(BoardError::Archived(card.id.clone()));
    }
    if to.kind == ColumnKind::Blocked {
        let has_reason = reason.is_some_and(|r| !r.trim().is_empty())
            || card
                .block_reason
                .as_deref()
                .is_some_and(|r| !r.trim().is_empty());
        if !has_reason {
            return Err(BoardError::ReasonRequired);
        }
    }
    Ok(())
}

/// Resultado de conferir o WIP de uma coluna para um cartão que vai entrar nela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wip {
    Fits,
    /// Coluna cheia (soma de todos).
    Full {
        count: u32,
        limit: u32,
    },
    /// O responsável já tem o máximo nesta coluna.
    FullForAgent {
        count: u32,
        limit: u32,
    },
}

/// Conta os cartões (não arquivados, fora o próprio) já na coluna.
pub fn check_wip(
    column: &Column,
    cards: &[Card],
    moving: &CardId,
    assignee: Option<&AgentId>,
) -> Wip {
    let here: Vec<&Card> = cards
        .iter()
        .filter(|c| c.column_id == column.id && c.archived_at.is_none() && &c.id != moving)
        .collect();
    if let Some(limit) = column.wip_limit {
        let count = u32::try_from(here.len()).unwrap_or(u32::MAX);
        if count >= limit {
            return Wip::Full { count, limit };
        }
    }
    if let (Some(limit), Some(agent)) = (column.wip_per_agent, assignee) {
        let count = u32::try_from(
            here.iter()
                .filter(|c| c.assignee.as_ref() == Some(agent))
                .count(),
        )
        .unwrap_or(u32::MAX);
        if count >= limit {
            return Wip::FullForAgent { count, limit };
        }
    }
    Wip::Fits
}

/// O caminho de `from` até `to` seguindo "depende de", se existir. Adicionar
/// `to → from` fecharia um ciclo exatamente quando este caminho existe.
pub fn dependency_path(
    edges: &[(CardId, CardId)],
    from: &CardId,
    to: &CardId,
) -> Option<Vec<CardId>> {
    let mut next: BTreeMap<&CardId, Vec<&CardId>> = BTreeMap::new();
    for (task, depends_on) in edges {
        next.entry(task).or_default().push(depends_on);
    }
    let mut stack = vec![vec![from.clone()]];
    let mut seen = BTreeSet::new();
    while let Some(path) = stack.pop() {
        let last = path.last()?.clone();
        if &last == to {
            return Some(path);
        }
        if !seen.insert(last.clone()) {
            continue;
        }
        for dep in next.get(&last).into_iter().flatten() {
            let mut longer = path.clone();
            longer.push((*dep).clone());
            stack.push(longer);
        }
    }
    None
}

/// Recusa `task → depends_on` se fechar um ciclo. `edges` são as dependências atuais.
pub fn check_dependency(
    edges: &[(CardId, CardId)],
    task: &CardId,
    depends_on: &CardId,
) -> Result<(), BoardError> {
    if task == depends_on {
        return Err(BoardError::DependencyCycle(vec![
            task.clone(),
            task.clone(),
        ]));
    }
    if let Some(mut path) = dependency_path(edges, depends_on, task) {
        // task → depends_on → ... → task
        path.insert(0, task.clone());
        return Err(BoardError::DependencyCycle(path));
    }
    Ok(())
}

/// Cartão "aberto" para fins de dependência: não arquivado e fora de coluna terminal.
pub fn is_open(card: &Card, columns: &[Column]) -> bool {
    card.archived_at.is_none()
        && columns
            .iter()
            .find(|c| c.id == card.column_id)
            .is_none_or(|c| c.kind != ColumnKind::Terminal)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::board::model::CardPriority;
    use crate::ids::TeamId;

    pub(crate) fn card(column: &Column) -> Card {
        Card {
            id: CardId::new(),
            team_id: TeamId::new(),
            column_id: column.id.clone(),
            title: "x".into(),
            body: String::new(),
            assignee: None,
            created_by: None,
            parent_id: None,
            position: 0,
            priority: CardPriority::Normal,
            labels: vec![],
            checklist: vec![],
            links: vec![],
            block_reason: None,
            version: 1,
            archived_at: None,
            approved_by: None,
            approved_at: None,
            column_since: 0,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn colunas_padrao_do_doc() {
        let columns = default_columns(&BoardId::new());
        let slugs: Vec<_> = columns.iter().map(|c| c.slug.as_str()).collect();
        assert_eq!(
            slugs,
            ["backlog", "todo", "doing", "blocked", "review", "done"]
        );
        assert_eq!(columns[2].wip_per_agent, Some(1));
        assert!(columns.iter().all(|c| valid_slug(&c.slug)));
        assert_eq!(
            columns.iter().map(|c| c.kind).collect::<Vec<_>>(),
            ColumnKind::ALL
        );
    }

    #[test]
    fn transicoes_invalidas_viram_erro_tipado() {
        let columns = default_columns(&BoardId::new());
        let c = card(&columns[1]);
        let err = check_transition(&c, &columns[3], None).unwrap_err();
        assert_eq!(err.code(), "reason_required");
        check_transition(&c, &columns[3], Some("aguarda @arquiteto")).unwrap();
        let err = column_by_slug(&columns, "em-progresso").unwrap_err();
        assert_eq!(err.code(), "unknown_column");
        assert!(err.to_string().contains("backlog, todo, doing"));
        let mut archived = c.clone();
        archived.archived_at = Some(1);
        assert_eq!(
            check_transition(&archived, &columns[2], None)
                .unwrap_err()
                .code(),
            "card_archived"
        );
    }

    #[test]
    fn wip_total_e_por_agente() {
        let mut columns = default_columns(&BoardId::new());
        columns[2].wip_limit = Some(2);
        let me = AgentId::new();
        let mut a = card(&columns[2]);
        a.assignee = Some(me.clone());
        let b = card(&columns[2]);
        let new = card(&columns[1]);
        assert_eq!(
            check_wip(&columns[2], &[a.clone()], &new.id, Some(&me)),
            Wip::FullForAgent { count: 1, limit: 1 }
        );
        assert_eq!(
            check_wip(&columns[2], &[a.clone(), b], &new.id, None),
            Wip::Full { count: 2, limit: 2 }
        );
        // O próprio cartão não conta contra ele.
        assert_eq!(
            check_wip(&columns[2], &[a.clone()], &a.id, Some(&me)),
            Wip::Fits
        );
    }

    #[test]
    fn ciclo_de_dependencia_e_recusado() {
        let (a, b, c) = (CardId::new(), CardId::new(), CardId::new());
        let edges = vec![(a.clone(), b.clone())];
        let err = check_dependency(&edges, &b, &a).unwrap_err();
        assert_eq!(err.code(), "dependency_cycle");
        assert_eq!(
            err.to_string(),
            format!("Isso criaria um ciclo: {b} → {a} → {b}.")
        );
        let edges = vec![(a.clone(), b.clone()), (b.clone(), c.clone())];
        assert!(check_dependency(&edges, &c, &a).is_err());
        check_dependency(&edges, &a, &c).unwrap();
        assert!(check_dependency(&edges, &a, &a).is_err());
    }
}
