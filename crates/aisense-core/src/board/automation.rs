//! Automações por coluna (`docs/13`, "Automações por coluna"): conjunto **fechado** de
//! gatilhos e ações. Nenhuma ação executa comando — isso é segurança, não limitação.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model::{Column, ColumnKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum Trigger {
    CardCreated,
    CardEnters,
    CardLeaves,
    CardStale,
    ChecklistComplete,
    CommentAdded,
}

impl Trigger {
    pub const ALL: [Trigger; 6] = [
        Self::CardCreated,
        Self::CardEnters,
        Self::CardLeaves,
        Self::CardStale,
        Self::ChecklistComplete,
        Self::CommentAdded,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CardCreated => "card_created",
            Self::CardEnters => "card_enters",
            Self::CardLeaves => "card_leaves",
            Self::CardStale => "card_stale",
            Self::ChecklistComplete => "checklist_complete",
            Self::CommentAdded => "comment_added",
        }
    }
}

/// Uma ação. O formato é o do TOML de `docs/13` (`{ assign = "@revisor" }`), por isso as
/// variantes se distinguem pela chave, não por uma tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum Action {
    /// `@handle` ou `actor` (quem disparou, se for agente).
    #[serde(rename_all = "camelCase")]
    Assign { assign: String },
    /// Destino: `@handle`, `@voce`, `assignee` ou `creator`.
    #[serde(rename_all = "camelCase")]
    Notify { notify: String, message: String },
    /// Slug da coluna de destino.
    Move {
        #[serde(rename = "move")]
        to: String,
    },
    #[serde(rename_all = "camelCase")]
    AddLabel {
        #[serde(rename = "add_label")]
        add_label: String,
    },
    /// Avisa os responsáveis dos cartões que dependiam deste e tira do bloqueio os que
    /// estavam parados só por ele.
    UnblockDependents {
        #[serde(rename = "unblock_dependents")]
        unblock_dependents: bool,
    },
    /// Cria um cartão novo (título) na coluna dada ou na primeira `ready`.
    CreateCard {
        #[serde(rename = "create_card")]
        create_card: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        column: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Automation {
    pub when: Trigger,
    /// Slug da coluna. Obrigatória para `card_enters`, `card_leaves` e `card_stale`;
    /// nos outros, restringe à coluna onde o cartão está.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    /// Horas parado para `card_stale` (aceita fração: 0.5 = 30 min).
    #[serde(default, rename = "after_h", skip_serializing_if = "Option::is_none")]
    pub after_h: Option<f64>,
    pub then: Vec<Action>,
}

/// O arquivo TOML que a UI mostra ("sem TOML na mão, mas com o TOML gerado à vista").
#[derive(Serialize, Deserialize)]
struct AutomationFile {
    #[serde(default)]
    automation: Vec<Automation>,
}

pub fn to_toml(automations: &[Automation]) -> String {
    toml::to_string(&AutomationFile {
        automation: automations.to_vec(),
    })
    .unwrap_or_default()
}

pub fn from_toml(source: &str) -> Result<Vec<Automation>, String> {
    toml::from_str::<AutomationFile>(source)
        .map(|f| f.automation)
        .map_err(|e| e.message().to_owned())
}

/// Confere uma automação contra as colunas do quadro. Devolve a mensagem para a UI.
pub fn validate(automation: &Automation, columns: &[Column]) -> Result<(), String> {
    let known = |slug: &str| columns.iter().any(|c| c.slug == slug);
    let needs_column = matches!(
        automation.when,
        Trigger::CardEnters | Trigger::CardLeaves | Trigger::CardStale
    );
    match &automation.column {
        Some(slug) if !known(slug) => return Err(format!("A coluna '{slug}' não existe.")),
        None if needs_column => {
            return Err(format!(
                "O gatilho {} precisa de uma coluna.",
                automation.when.as_str()
            ))
        }
        _ => {}
    }
    if automation.when == Trigger::CardStale {
        match automation.after_h {
            Some(h) if h > 0.0 && h.is_finite() => {}
            _ => return Err("card_stale precisa de after_h maior que zero.".into()),
        }
    }
    if automation.then.is_empty() {
        return Err("A automação não faz nada: inclua ao menos uma ação.".into());
    }
    for action in &automation.then {
        match action {
            Action::Assign { assign } if assign != "actor" && !assign.starts_with('@') => {
                return Err(format!(
                    "assign espera @handle ou actor, recebeu '{assign}'."
                ))
            }
            Action::Notify { notify, message } => {
                let ok = notify.starts_with('@') || notify == "assignee" || notify == "creator";
                if !ok {
                    return Err(format!(
                        "notify espera @handle, assignee ou creator, recebeu '{notify}'."
                    ));
                }
                if message.trim().is_empty() {
                    return Err("notify precisa de uma mensagem.".into());
                }
            }
            Action::Move { to } if !known(to) => {
                return Err(format!("move: a coluna '{to}' não existe."))
            }
            Action::AddLabel { add_label }
                if super::model::normalize_label(add_label).is_none() =>
            {
                return Err(format!("Label inválida: '{add_label}'."))
            }
            Action::CreateCard {
                create_card,
                column,
            } => {
                if create_card.trim().is_empty() {
                    return Err("create_card precisa de um título.".into());
                }
                if let Some(slug) = column.as_deref().filter(|s| !known(s)) {
                    return Err(format!("create_card: a coluna '{slug}' não existe."));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Automações com que toda equipe nasce. `reviewer` é o handle de quem revisa, se a equipe
/// tiver um (`@revisor`); sem ninguém, o aviso vai para você.
pub fn defaults(columns: &[Column], reviewer: Option<&str>) -> Vec<Automation> {
    let slug_of = |kind: ColumnKind| {
        columns
            .iter()
            .find(|c| c.kind == kind)
            .map(|c| c.slug.clone())
    };
    let mut out = Vec::new();
    if let Some(active) = slug_of(ColumnKind::Active) {
        out.push(Automation {
            when: Trigger::CardEnters,
            column: Some(active.clone()),
            after_h: None,
            then: vec![Action::Assign {
                assign: "actor".into(),
            }],
        });
        out.push(Automation {
            when: Trigger::CardStale,
            column: Some(active),
            after_h: Some(4.0),
            then: vec![Action::Notify {
                notify: "assignee".into(),
                message: "cartão parado há 4h em Fazendo: conclua, bloqueie com motivo ou devolva"
                    .into(),
            }],
        });
    }
    if let Some(review) = slug_of(ColumnKind::Review) {
        let who = reviewer.map_or_else(|| "@voce".to_owned(), |h| format!("@{h}"));
        out.push(Automation {
            when: Trigger::CardEnters,
            column: Some(review),
            after_h: None,
            then: vec![Action::Notify {
                notify: who,
                message: "cartão pronto para revisão".into(),
            }],
        });
    }
    if let Some(done) = slug_of(ColumnKind::Terminal) {
        out.push(Automation {
            when: Trigger::CardEnters,
            column: Some(done),
            after_h: None,
            then: vec![Action::UnblockDependents {
                unblock_dependents: true,
            }],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::board::default_columns;
    use crate::ids::BoardId;

    #[test]
    fn toml_do_doc_volta_igual() {
        let source = r#"
[[automation]]
when   = "card_enters"
column = "review"
then   = [
  { assign = "@revisor" },
  { notify = "@revisor", message = "cartão pronto para revisão" },
]

[[automation]]
when     = "card_stale"
column   = "doing"
after_h  = 4
then     = [{ notify = "@coordenador", message = "cartão parado há 4h" }]

[[automation]]
when   = "card_enters"
column = "done"
then   = [{ unblock_dependents = true }, { move = "done" }, { add_label = "entregue" }]
"#;
        let parsed = from_toml(source).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(
            parsed[0].then[0],
            Action::Assign {
                assign: "@revisor".into()
            }
        );
        assert_eq!(parsed[1].after_h, Some(4.0));
        assert_eq!(parsed[2].then[1], Action::Move { to: "done".into() });
        assert_eq!(from_toml(&to_toml(&parsed)).unwrap(), parsed);
        let columns = default_columns(&BoardId::new());
        for a in &parsed {
            validate(a, &columns).unwrap();
        }
    }

    #[test]
    fn conjunto_fechado_e_validado() {
        // Ação desconhecida (ex.: rodar comando) não é aceita.
        assert!(from_toml("[[automation]]\nwhen = \"card_enters\"\ncolumn = \"doing\"\nthen = [{ run = \"rm -rf /\" }]").is_err());
        assert!(from_toml("[[automation]]\nwhen = \"on_push\"\nthen = []").is_err());
        let columns = default_columns(&BoardId::new());
        let bad = Automation {
            when: Trigger::CardEnters,
            column: Some("em-progresso".into()),
            after_h: None,
            then: vec![],
        };
        assert!(validate(&bad, &columns).unwrap_err().contains("não existe"));
        let stale = Automation {
            when: Trigger::CardStale,
            column: Some("doing".into()),
            after_h: None,
            then: vec![Action::AddLabel {
                add_label: "parado".into(),
            }],
        };
        assert!(validate(&stale, &columns).unwrap_err().contains("after_h"));
        for a in defaults(&columns, Some("revisor")) {
            validate(&a, &columns).unwrap();
        }
    }
}
