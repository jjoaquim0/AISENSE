//! Entidade `Team` (`docs/04`, tabela `teams`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agent::WorkspaceMode;
use crate::color::AgentColor;
use crate::ids::TeamId;
use crate::time::Millis;
use crate::validation::{optional_text, required_text, ValidationError};

pub const TEAM_NAME_MAX: usize = 64;
pub const TEAM_MISSION_MAX: usize = 8_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Team {
    pub id: TeamId,
    pub name: String,
    /// Injetada no bootstrap de todos os agentes.
    pub mission: String,
    /// Diretório de trabalho padrão dos agentes.
    pub workdir: String,
    pub color: AgentColor,
    pub icon: Option<String>,
    pub workspace_mode: WorkspaceMode,
    /// Estado da Sala da Equipe (modo de vista, painéis). Opaco para o domínio.
    #[ts(type = "Record<string, unknown>")]
    pub layout: serde_json::Value,
    #[ts(type = "number | null")]
    pub archived_at: Option<Millis>,
    #[ts(type = "number")]
    pub created_at: Millis,
    #[ts(type = "number")]
    pub updated_at: Millis,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct TeamDraft {
    pub name: String,
    pub mission: String,
    pub workdir: String,
    pub color: AgentColor,
    #[ts(optional)]
    pub icon: Option<String>,
    pub workspace_mode: WorkspaceMode,
}

impl Team {
    pub fn create(draft: &TeamDraft, now: Millis) -> Result<Self, ValidationError> {
        let (name, mission, workdir, icon) = validate(draft)?;
        Ok(Self {
            id: TeamId::new(),
            name,
            mission,
            workdir,
            color: draft.color,
            icon,
            workspace_mode: draft.workspace_mode,
            layout: serde_json::Value::Object(serde_json::Map::new()),
            archived_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn apply(&mut self, draft: &TeamDraft, now: Millis) -> Result<(), ValidationError> {
        let (name, mission, workdir, icon) = validate(draft)?;
        self.name = name;
        self.mission = mission;
        self.workdir = workdir;
        self.color = draft.color;
        self.icon = icon;
        self.workspace_mode = draft.workspace_mode;
        self.updated_at = now;
        Ok(())
    }

    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    pub fn to_draft(&self) -> TeamDraft {
        TeamDraft {
            name: self.name.clone(),
            mission: self.mission.clone(),
            workdir: self.workdir.clone(),
            color: self.color,
            icon: self.icon.clone(),
            workspace_mode: self.workspace_mode,
        }
    }
}

type ValidTeam = (String, String, String, Option<String>);

fn validate(draft: &TeamDraft) -> Result<ValidTeam, ValidationError> {
    let name = required_text("name", &draft.name, TEAM_NAME_MAX)?;
    let workdir = required_text("workdir", &draft.workdir, 4_096)?;
    let mission = draft.mission.trim().to_owned();
    if mission.chars().count() > TEAM_MISSION_MAX {
        return Err(ValidationError::TooLong {
            field: "mission",
            max: TEAM_MISSION_MAX,
        });
    }
    let icon = optional_text("icon", draft.icon.as_deref(), 64)?;
    Ok((name, mission, workdir, icon))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn draft() -> TeamDraft {
        TeamDraft {
            name: "  Squad Produto ".into(),
            workdir: "/home/dev/app".into(),
            ..TeamDraft::default()
        }
    }

    #[test]
    fn creates_trimmed_with_defaults() {
        let t = Team::create(&draft(), 5).unwrap();
        assert_eq!(t.name, "Squad Produto");
        assert_eq!(t.color, AgentColor::Violet);
        assert_eq!(t.workspace_mode, WorkspaceMode::Shared);
        assert_eq!(t.layout, serde_json::json!({}));
        assert!(!t.is_archived());
    }

    #[test]
    fn requires_name_and_workdir() {
        let mut d = draft();
        d.name.clear();
        assert_eq!(
            Team::create(&d, 0),
            Err(ValidationError::Empty { field: "name" })
        );
        let mut d = draft();
        d.workdir = " ".into();
        assert_eq!(
            Team::create(&d, 0),
            Err(ValidationError::Empty { field: "workdir" })
        );
        let mut d = draft();
        d.name = "x".repeat(TEAM_NAME_MAX + 1);
        assert_eq!(
            Team::create(&d, 0),
            Err(ValidationError::TooLong {
                field: "name",
                max: TEAM_NAME_MAX
            })
        );
    }

    #[test]
    fn apply_updates_timestamp_and_keeps_identity() {
        let mut t = Team::create(&draft(), 5).unwrap();
        let id = t.id.clone();
        let mut d = t.to_draft();
        d.mission = "Entregar o checkout".into();
        t.apply(&d, 9).unwrap();
        assert_eq!(t.id, id);
        assert_eq!(t.mission, "Entregar o checkout");
        assert_eq!((t.created_at, t.updated_at), (5, 9));
    }
}
