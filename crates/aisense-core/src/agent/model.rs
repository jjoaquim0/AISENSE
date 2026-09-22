//! Entidade `Agent` (`docs/04`, tabela `agents`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Autonomy, DeliveryMode, Handle, RestartPolicy, Workbench};
use crate::color::AgentColor;
use crate::ids::{AgentId, TeamId};
use crate::time::Millis;
use crate::validation::{optional_text, required_text, ValidationError};

pub const AGENT_NAME_MAX: usize = 64;
pub const AGENT_ROLE_MAX: usize = 8_000;
pub const ADAPTER_ID_MAX: usize = 64;

/// Prefixo das variáveis que o supervisor injeta (`AISENSE_TOKEN`, `AISENSE_AGENT_ID`...).
/// Deixar o usuário sobrescrevê-las permitiria um agente se passar por outro no barramento.
pub const RESERVED_ENV_PREFIX: &str = "AISENSE_";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Agent {
    pub id: AgentId,
    pub team_id: TeamId,
    pub handle: Handle,
    pub name: String,
    pub role: String,
    pub adapter_id: String,
    pub model: Option<String>,
    /// `None` herda o diretório da equipe.
    pub workdir: Option<String>,
    pub env: BTreeMap<String, String>,
    pub args: Vec<String>,
    pub color: AgentColor,
    pub autostart: bool,
    pub restart_policy: RestartPolicy,
    pub delivery_mode: DeliveryMode,
    pub autonomy: Autonomy,
    pub workbench: Workbench,
    pub position: i32,
    #[ts(type = "number")]
    pub created_at: Millis,
    #[ts(type = "number")]
    pub updated_at: Millis,
}

/// O que a UI envia para criar ou editar um agente. Campos opcionais assumem os
/// padrões do esquema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentDraft {
    pub handle: String,
    pub name: String,
    pub role: String,
    pub adapter_id: String,
    #[ts(optional)]
    pub model: Option<String>,
    #[ts(optional)]
    pub workdir: Option<String>,
    pub env: BTreeMap<String, String>,
    pub args: Vec<String>,
    /// `None` = próxima cor livre da equipe.
    #[ts(optional)]
    pub color: Option<AgentColor>,
    pub autostart: bool,
    pub restart_policy: RestartPolicy,
    pub delivery_mode: DeliveryMode,
    pub autonomy: Autonomy,
    pub workbench: Workbench,
}

/// Campos validados de um rascunho, prontos para virar ou atualizar um `Agent`.
struct Validated {
    handle: Handle,
    name: String,
    role: String,
    adapter_id: String,
    model: Option<String>,
    workdir: Option<String>,
    env: BTreeMap<String, String>,
}

impl AgentDraft {
    fn validate(&self) -> Result<Validated, ValidationError> {
        let role = self.role.trim().to_owned();
        if role.chars().count() > AGENT_ROLE_MAX {
            return Err(ValidationError::TooLong {
                field: "role",
                max: AGENT_ROLE_MAX,
            });
        }
        Ok(Validated {
            handle: Handle::parse(self.handle.trim())?,
            name: required_text("name", &self.name, AGENT_NAME_MAX)?,
            role,
            adapter_id: required_text("adapterId", &self.adapter_id, ADAPTER_ID_MAX)?,
            model: optional_text("model", self.model.as_deref(), 128)?,
            workdir: optional_text("workdir", self.workdir.as_deref(), 4_096)?,
            env: validate_env(&self.env)?,
        })
    }
}

impl Agent {
    /// Cria um agente novo em `team`. `existing` são os agentes já na equipe: com
    /// eles o core garante handle único (I1) e escolhe a cor e a posição.
    pub fn create(
        team_id: TeamId,
        draft: &AgentDraft,
        existing: &[Agent],
        now: Millis,
    ) -> Result<Self, ValidationError> {
        let v = draft.validate()?;
        ensure_unique_handle(&v.handle, None, existing)?;
        let used: Vec<AgentColor> = existing.iter().map(|a| a.color).collect();
        let position = existing
            .iter()
            .map(|a| a.position)
            .max()
            .map_or(0, |p| p.saturating_add(1));
        Ok(Self {
            id: AgentId::new(),
            team_id,
            handle: v.handle,
            name: v.name,
            role: v.role,
            adapter_id: v.adapter_id,
            model: v.model,
            workdir: v.workdir,
            env: v.env,
            args: draft.args.clone(),
            color: draft.color.unwrap_or_else(|| AgentColor::next_free(&used)),
            autostart: draft.autostart,
            restart_policy: draft.restart_policy,
            delivery_mode: draft.delivery_mode,
            autonomy: draft.autonomy,
            workbench: draft.workbench,
            position,
            created_at: now,
            updated_at: now,
        })
    }

    /// Aplica uma edição. `siblings` são os agentes da equipe (pode incluir este).
    pub fn apply(
        &mut self,
        draft: &AgentDraft,
        siblings: &[Agent],
        now: Millis,
    ) -> Result<(), ValidationError> {
        let v = draft.validate()?;
        ensure_unique_handle(&v.handle, Some(&self.id), siblings)?;
        self.handle = v.handle;
        self.name = v.name;
        self.role = v.role;
        self.adapter_id = v.adapter_id;
        self.model = v.model;
        self.workdir = v.workdir;
        self.env = v.env;
        self.args = draft.args.clone();
        if let Some(color) = draft.color {
            self.color = color;
        }
        self.autostart = draft.autostart;
        self.restart_policy = draft.restart_policy;
        self.delivery_mode = draft.delivery_mode;
        self.autonomy = draft.autonomy;
        self.workbench = draft.workbench;
        self.updated_at = now;
        Ok(())
    }

    /// Rascunho equivalente, para o formulário de edição começar preenchido.
    pub fn to_draft(&self) -> AgentDraft {
        AgentDraft {
            handle: self.handle.as_str().to_owned(),
            name: self.name.clone(),
            role: self.role.clone(),
            adapter_id: self.adapter_id.clone(),
            model: self.model.clone(),
            workdir: self.workdir.clone(),
            env: self.env.clone(),
            args: self.args.clone(),
            color: Some(self.color),
            autostart: self.autostart,
            restart_policy: self.restart_policy,
            delivery_mode: self.delivery_mode,
            autonomy: self.autonomy,
            workbench: self.workbench,
        }
    }
}

/// I1: handle único na equipe. `except` ignora o próprio agente numa edição.
pub fn ensure_unique_handle(
    handle: &Handle,
    except: Option<&AgentId>,
    team_agents: &[Agent],
) -> Result<(), ValidationError> {
    let taken = team_agents
        .iter()
        .any(|a| a.handle == *handle && Some(&a.id) != except);
    if taken {
        Err(ValidationError::DuplicateHandle(handle.as_str().to_owned()))
    } else {
        Ok(())
    }
}

/// Nome de variável POSIX e fora do prefixo reservado.
fn validate_env(
    env: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, ValidationError> {
    for key in env.keys() {
        let mut chars = key.chars();
        let valid = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            return Err(ValidationError::InvalidEnvKey(key.clone()));
        }
        if key.to_ascii_uppercase().starts_with(RESERVED_ENV_PREFIX) {
            return Err(ValidationError::ReservedEnvKey(key.clone()));
        }
    }
    Ok(env.clone())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn draft(handle: &str) -> AgentDraft {
        AgentDraft {
            handle: handle.into(),
            name: "Backend".into(),
            adapter_id: "shell".into(),
            ..AgentDraft::default()
        }
    }

    fn create(handle: &str, existing: &[Agent]) -> Result<Agent, ValidationError> {
        Agent::create(TeamId::from_raw("tem_x"), &draft(handle), existing, 1_000)
    }

    #[test]
    fn creates_with_schema_defaults() {
        let a = create("backend", &[]).unwrap();
        assert_eq!(a.handle.as_str(), "backend");
        assert_eq!(a.restart_policy, RestartPolicy::OnCrash);
        assert_eq!(a.delivery_mode, DeliveryMode::Pull);
        assert_eq!(a.autonomy, Autonomy::Ask);
        assert_eq!(a.color, AgentColor::Violet);
        assert_eq!(a.position, 0);
        assert_eq!((a.created_at, a.updated_at), (1_000, 1_000));
    }

    #[test]
    fn next_agent_gets_next_color_and_position() {
        let first = create("backend", &[]).unwrap();
        let second = create("frontend", &[first]).unwrap();
        assert_eq!(second.color, AgentColor::Cyan);
        assert_eq!(second.position, 1);
    }

    #[test]
    fn rejects_duplicate_handle_in_the_same_team() {
        let first = create("backend", &[]).unwrap();
        assert_eq!(
            create("backend", &[first]),
            Err(ValidationError::DuplicateHandle("backend".into()))
        );
    }

    #[test]
    fn rejects_invalid_and_reserved_handles() {
        assert!(matches!(
            create("Back End", &[]),
            Err(ValidationError::InvalidHandle { .. })
        ));
        assert_eq!(
            create("all", &[]),
            Err(ValidationError::ReservedHandle("all".into()))
        );
        assert_eq!(
            create("voce", &[]),
            Err(ValidationError::ReservedHandle("voce".into()))
        );
    }

    #[test]
    fn requires_name_and_adapter() {
        let mut d = draft("backend");
        d.name = "   ".into();
        assert_eq!(
            Agent::create(TeamId::new(), &d, &[], 0),
            Err(ValidationError::Empty { field: "name" })
        );
        let mut d = draft("backend");
        d.adapter_id.clear();
        assert_eq!(
            Agent::create(TeamId::new(), &d, &[], 0),
            Err(ValidationError::Empty { field: "adapterId" })
        );
    }

    #[test]
    fn refuses_env_that_would_impersonate_another_agent() {
        let mut d = draft("backend");
        d.env.insert("AISENSE_TOKEN".into(), "stolen".into());
        assert_eq!(
            Agent::create(TeamId::new(), &d, &[], 0),
            Err(ValidationError::ReservedEnvKey("AISENSE_TOKEN".into()))
        );
        let mut d = draft("backend");
        d.env.insert("aisense_agent_id".into(), "x".into());
        assert!(matches!(
            Agent::create(TeamId::new(), &d, &[], 0),
            Err(ValidationError::ReservedEnvKey(_))
        ));
        let mut d = draft("backend");
        d.env.insert("1BAD".into(), "x".into());
        assert_eq!(
            Agent::create(TeamId::new(), &d, &[], 0),
            Err(ValidationError::InvalidEnvKey("1BAD".into()))
        );
    }

    #[test]
    fn editing_keeps_own_handle_but_not_a_siblings() {
        let a = create("backend", &[]).unwrap();
        let b = create("frontend", std::slice::from_ref(&a)).unwrap();
        let team = vec![a.clone(), b.clone()];

        let mut edited = a.clone();
        let mut d = a.to_draft();
        d.name = "Backend Sênior".into();
        edited.apply(&d, &team, 2_000).unwrap();
        assert_eq!(edited.name, "Backend Sênior");
        assert_eq!(edited.updated_at, 2_000);
        assert_eq!(edited.created_at, a.created_at);

        d.handle = "frontend".into();
        assert_eq!(
            edited.apply(&d, &team, 3_000),
            Err(ValidationError::DuplicateHandle("frontend".into()))
        );
        assert_eq!(
            edited.handle.as_str(),
            "backend",
            "a failed edit changes nothing"
        );
    }

    #[test]
    fn draft_round_trips() {
        let a = create("backend", &[]).unwrap();
        let mut again = a.clone();
        again
            .apply(&a.to_draft(), std::slice::from_ref(&a), a.updated_at)
            .unwrap();
        assert_eq!(again, a);
    }
}
