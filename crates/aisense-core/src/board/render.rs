//! `aisense board` e `aisense task show` em texto (`docs/13`, "Leitura"): pensado para um
//! modelo entender de primeira e caber no contexto. Mesma função na CLI e no MCP.

use super::model::Comment;
use super::model::{Actor, ColumnKind};
use super::service::{AgentTag, BoardView, CardDetail, CardView, Moved};
use crate::time::Millis;

/// Quantos cartões cada coluna mostra antes de resumir ("… e mais N"). `--full` tira o teto.
pub const BOARD_COLUMN_LINES: usize = 8;
/// Colunas terminais mostram só os mais recentes.
const DONE_LINES: usize = 3;

/// `tsk_` + o fim do ULID: curto para ler, e `aisense task` aceita assim.
pub fn short_id(id: &str) -> String {
    match id.strip_prefix("tsk_") {
        Some(ulid) if ulid.len() > 6 => format!("tsk_{}", &ulid[ulid.len() - 6..]),
        _ => id.to_owned(),
    }
}

/// "agora", "há 5min", "há 3h", "há 2d".
pub fn ago(now: Millis, then: Millis) -> String {
    let secs = (now - then).max(0) / 1000;
    match secs {
        0..=59 => "agora".into(),
        60..=3599 => format!("há {}min", secs / 60),
        3600..=86_399 => format!("há {}h", secs / 3600),
        _ => format!("há {}d", secs / 86_400),
    }
}

fn truncate(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("");
    if line.chars().count() <= max {
        line.to_owned()
    } else {
        let cut: String = line.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

fn pad(text: &str, width: usize) -> String {
    let n = text.chars().count();
    if n >= width {
        text.to_owned()
    } else {
        format!("{text}{}", " ".repeat(width - n))
    }
}

fn card_line(card: &CardView, kind: ColumnKind, now: Millis) -> String {
    let c = &card.card;
    let priority = format!("[{}]", c.priority.label());
    let who = card
        .assignee_handle
        .as_ref()
        .map_or_else(|| "sem responsável".to_owned(), |h| format!("@{h}"));
    let mut tail = match kind {
        ColumnKind::Blocked => format!(
            "motivo: {}",
            c.block_reason
                .as_deref()
                .map_or("—", |r| r.lines().next().unwrap_or(""))
        ),
        ColumnKind::Active | ColumnKind::Review => {
            format!("{who}   {}", ago(now, c.column_since))
        }
        ColumnKind::Terminal => format!("{who}   {}", ago(now, c.column_since)),
        _ if !card.blocked_by.is_empty() => format!(
            "bloqueado por {}",
            card.blocked_by
                .iter()
                .map(|d| short_id(d.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => who,
    };
    let (done, total) = c.checklist_progress();
    if total > 0 && kind != ColumnKind::Terminal {
        tail.push_str(&format!("   ✓{done}/{total}"));
    }
    format!(
        "  {}  {} {}  {}",
        short_id(c.id.as_str()),
        pad(&priority, 9),
        pad(&truncate(&c.title, 44), 44),
        tail
    )
}

/// O quadro inteiro. `column` restringe a uma coluna; `full` mostra todos os cartões.
pub fn render_board(view: &BoardView, column: Option<&str>, full: bool) -> String {
    let updated = view
        .cards
        .iter()
        .map(|c| c.card.updated_at)
        .max()
        .map_or_else(
            || "sem cartões".to_owned(),
            |t| format!("atualizado {}", ago(view.now, t)),
        );
    let title = format!("QUADRO — {}", view.team_name);
    let mut out = format!("{}{updated}\n", pad(&title, 48));
    for col in &view.columns {
        if column.is_some_and(|slug| slug != col.slug) {
            continue;
        }
        let mut cards: Vec<&CardView> = view
            .cards
            .iter()
            .filter(|c| c.card.column_id == col.id)
            .collect();
        let count = cards.len();
        let limit = match col.wip_limit {
            Some(l) => format!("{count}/{l}"),
            None => count.to_string(),
        };
        out.push_str(&format!(
            "\n{} ({limit}) · {}\n",
            col.name.to_uppercase(),
            col.slug
        ));
        if cards.is_empty() {
            continue;
        }
        let cap = if full {
            usize::MAX
        } else if col.kind == ColumnKind::Terminal {
            cards.sort_by_key(|c| std::cmp::Reverse(c.card.column_since));
            DONE_LINES
        } else {
            BOARD_COLUMN_LINES
        };
        for card in cards.iter().take(cap) {
            out.push_str(&card_line(card, col.kind, view.now));
            out.push('\n');
        }
        if count > cap {
            out.push_str(&format!(
                "  … e mais {} (aisense board --column {} --full)\n",
                count - cap,
                col.slug
            ));
        }
    }
    out
}

/// `aisense task list` e `task next`.
pub fn render_cards(cards: &[CardView], now: Millis) -> String {
    if cards.is_empty() {
        return "Nenhum cartão.\n".into();
    }
    cards
        .iter()
        .map(|c| {
            format!(
                "{}  ({})\n",
                card_line(c, ColumnKind::Ready, now).trim_start(),
                c.column_slug
            )
        })
        .collect()
}

fn who(actor: &Actor, agents: &[AgentTag]) -> String {
    match actor {
        Actor::Human => "@voce".into(),
        Actor::System => "AISENSE".into(),
        Actor::Agent { agent_id } => agents.iter().find(|a| &a.id == agent_id).map_or_else(
            || "agente removido".into(),
            |a| format!("@{}", a.handle.as_str()),
        ),
    }
}

/// `aisense task show`: tudo do cartão, histórico no fim.
pub fn render_card(detail: &CardDetail, now: Millis) -> String {
    let view = &detail.card;
    let c = &view.card;
    let mut out = format!("{} — {}\n", c.id, c.title);
    out.push_str(&format!(
        "coluna: {} ({}) · prioridade: {} · responsável: {}\n",
        detail.column.name,
        detail.column.slug,
        c.priority.label(),
        view.assignee_handle
            .as_ref()
            .map_or_else(|| "ninguém".to_owned(), |h| format!("@{h}"))
    ));
    if !c.labels.is_empty() {
        out.push_str(&format!("labels: {}\n", c.labels.join(", ")));
    }
    if let Some(reason) = &c.block_reason {
        out.push_str(&format!("bloqueado: {reason}\n"));
    }
    if c.approved_at.is_some() {
        out.push_str(&format!(
            "aprovado por {}\n",
            c.approved_by.as_ref().map_or_else(
                || "@voce".to_owned(),
                |id| who(
                    &Actor::Agent {
                        agent_id: id.clone()
                    },
                    &detail.agents
                )
            )
        ));
    }
    if !c.body.trim().is_empty() {
        out.push_str(&format!("\n{}\n", c.body.trim_end()));
    }
    if !c.checklist.is_empty() {
        out.push_str("\nchecklist:\n");
        for (i, item) in c.checklist.iter().enumerate() {
            let mark = if item.done { "x" } else { " " };
            out.push_str(&format!("  {}. [{mark}] {}\n", i + 1, item.text));
        }
    }
    let refs = |title: &str, list: &[super::service::CardRef], out: &mut String| {
        if list.is_empty() {
            return;
        }
        out.push_str(&format!("\n{title}:\n"));
        for r in list {
            let state = if r.open { "aberto" } else { "concluído" };
            out.push_str(&format!(
                "  {}  {} ({}, {state})\n",
                short_id(r.id.as_str()),
                truncate(&r.title, 60),
                r.column_slug
            ));
        }
    };
    refs("depende de", &detail.depends_on, &mut out);
    refs("bloqueia", &detail.dependents, &mut out);
    refs("subtarefas", &detail.children, &mut out);
    if !c.links.is_empty() {
        out.push_str("\nlinks:\n");
        for link in &c.links {
            let kind = serde_json::to_value(link.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            out.push_str(&format!("  {kind}: {}\n", link.target));
        }
    }
    if !detail.comments.is_empty() {
        out.push_str("\ncomentários:\n");
        for comment in &detail.comments {
            out.push_str(&format!(
                "── {} · {}\n{}\n",
                who(&comment.author, &detail.agents),
                ago(now, comment.created_at),
                comment.body.trim_end()
            ));
        }
    }
    if !detail.activity.is_empty() {
        out.push_str("\nhistórico:\n");
        for a in &detail.activity {
            out.push_str(&format!(
                "  {} {} {}{}\n",
                ago(now, a.created_at),
                who(&a.actor, &detail.agents),
                a.action,
                activity_detail(&a.detail)
            ));
        }
    }
    out
}

fn activity_detail(detail: &serde_json::Value) -> String {
    let s = |k: &str| detail.get(k).and_then(|v| v.as_str());
    match (s("from"), s("to")) {
        (Some(from), Some(to)) => {
            let mut text = format!(" {from} → {to}");
            if let Some(reason) = s("reason") {
                text.push_str(&format!(" ({reason})"));
            }
            text
        }
        _ => match s("command") {
            Some(command) => format!(" '{command}'"),
            None => String::new(),
        },
    }
}

/// Resultado de uma operação que muda cartão: onde ficou, com quem, e os avisos.
pub fn render_moved(moved: &Moved) -> String {
    let view = &moved.card;
    let c = &view.card;
    let who = view
        .assignee_handle
        .as_ref()
        .map_or_else(|| "ninguém".to_owned(), |h| format!("@{h}"));
    let mut out = format!(
        "{} · {}\n  coluna: {} · responsável: {who}",
        short_id(c.id.as_str()),
        truncate(&c.title, 80),
        view.column_slug
    );
    let (done, total) = c.checklist_progress();
    if total > 0 {
        out.push_str(&format!(" · checklist {done}/{total}"));
    }
    out.push('\n');
    for warning in &moved.warnings {
        out.push_str(&format!("aviso: {warning}\n"));
    }
    out
}

/// `aisense task next`: o cartão e o próximo passo.
pub fn render_next(card: Option<&CardView>, now: Millis) -> String {
    match card {
        None => "Nada pronto para você agora. Espere trabalho com: aisense task watch\n".into(),
        Some(card) => {
            let mut out = render_cards(std::slice::from_ref(card), now);
            if card.card.assignee.is_none() {
                out.push_str(&format!(
                    "Pegue com: aisense task claim {}\n",
                    short_id(card.card.id.as_str())
                ));
            } else {
                out.push_str(&format!(
                    "Já é seu. Comece com: aisense task move {} doing\n",
                    short_id(card.card.id.as_str())
                ));
            }
            out
        }
    }
}

pub fn render_comment(comment: &Comment) -> String {
    format!(
        "comentado em {} ({})\n",
        short_id(comment.card_id.as_str()),
        comment.id
    )
}
