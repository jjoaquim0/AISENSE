//! Quais skills um agente leva ao subir (F04-03): as habilitadas, que existem no disco e
//! rodam no runtime dele, em ordem de `priority` e depois de `position` (`docs/06`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog::{SkillCatalog, TEAMWORK_SKILL};
use super::model::Skill;
use crate::ids::AgentId;
use crate::repo::{RepoResult, SkillRepository};

/// O que entra no boot e o que fica de fora (com o porquê).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedSkills {
    /// Na ordem de injeção.
    pub active: Vec<Skill>,
    pub ignored: Vec<IgnoredSkill>,
}

impl ResolvedSkills {
    /// A versão para a UI: nomes, sem o conteúdo.
    pub fn plan(&self) -> SkillPlan {
        SkillPlan {
            active: self.active.iter().map(|s| s.name.clone()).collect(),
            ignored: self.ignored.clone(),
        }
    }
}

/// Resumo mostrado na UI e devolvido pelo start: o que o agente leva no próximo boot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SkillPlan {
    /// Nomes, na ordem de injeção.
    pub active: Vec<String>,
    pub ignored: Vec<IgnoredSkill>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct IgnoredSkill {
    pub name: String,
    pub reason: IgnoreReason,
    /// Frase pronta para a UI, em pt-BR (como os avisos de bancada).
    pub message: String,
}

impl IgnoredSkill {
    fn new(name: &str, reason: IgnoreReason) -> Self {
        let message = match &reason {
            IgnoreReason::Incompatible {
                adapter_id,
                targets,
            } => format!(
                "skill {name} ignorada: não roda em {adapter_id} (só {})",
                targets.join(", ")
            ),
            IgnoreReason::Missing => {
                format!("skill {name} ignorada: o SKILL.md não está no disco ou não carrega")
            }
        };
        Self {
            name: name.to_owned(),
            reason,
            message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum IgnoreReason {
    /// `targets` não inclui o runtime do agente.
    #[serde(rename_all = "camelCase")]
    Incompatible {
        adapter_id: String,
        targets: Vec<String>,
    },
    /// Atribuída, mas o `SKILL.md` não está (mais) no disco ou não carrega.
    Missing,
}

impl std::fmt::Display for IgnoredSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.reason {
            IgnoreReason::Incompatible {
                adapter_id,
                targets,
            } => write!(
                f,
                "{} does not run on {adapter_id} (only {})",
                self.name,
                targets.join(", ")
            ),
            IgnoreReason::Missing => write!(f, "{} is not on disk", self.name),
        }
    }
}

/// Resolve as skills do agente contra o catálogo atual. Desabilitadas nem aparecem.
pub async fn resolve_agent_skills<S: SkillRepository>(
    store: &S,
    catalog: &SkillCatalog,
    agent_id: &AgentId,
    adapter_id: &str,
) -> RepoResult<ResolvedSkills> {
    let assigned = store.agent_skills(agent_id).await?;
    let records = store.list_skills().await?;
    let mut resolved = ResolvedSkills::default();
    for entry in assigned.iter().filter(|e| e.enabled) {
        let Some(record) = records.iter().find(|r| r.id == entry.skill_id) else {
            continue;
        };
        // Já vai em todo BOOT.md, em seção própria; atribuída, entraria duas vezes.
        if record.slug == TEAMWORK_SKILL {
            continue;
        }
        match catalog.get(&record.slug) {
            None => resolved
                .ignored
                .push(IgnoredSkill::new(&record.slug, IgnoreReason::Missing)),
            Some(skill) if !skill.supports(adapter_id) => resolved.ignored.push(IgnoredSkill::new(
                &skill.name,
                IgnoreReason::Incompatible {
                    adapter_id: adapter_id.to_owned(),
                    targets: skill.targets.clone(),
                },
            )),
            Some(skill) => resolved.active.push(skill.clone()),
        }
    }
    // Estável: com a mesma prioridade, vale a ordem que o usuário deu.
    resolved.active.sort_by_key(|s| s.priority);
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::agent::{create_agent, AgentDraft};
    use crate::repo::{AgentSkill, InMemoryStore, TeamRepository};
    use crate::skill::catalog::BuiltinSkill;
    use crate::team::{Team, TeamDraft};

    fn md(name: &str, extra: &str) -> String {
        format!("---\nname: {name}\ndescription: Faz {name}.\n{extra}---\ncorpo de {name}\n")
    }

    async fn setup(
        adapter: &str,
        skills: &[(&'static str, String)],
    ) -> (
        InMemoryStore,
        SkillCatalog,
        AgentId,
        Vec<crate::repo::SkillRecord>,
    ) {
        let store = InMemoryStore::new();
        let team = Team::create(
            &TeamDraft {
                name: "Squad".into(),
                workdir: "/tmp".into(),
                ..TeamDraft::default()
            },
            1,
        )
        .unwrap();
        store.create_team(&team).await.unwrap();
        let draft = AgentDraft {
            handle: "revisor".into(),
            name: "Revisor".into(),
            adapter_id: adapter.into(),
            ..AgentDraft::default()
        };
        let agent = create_agent(&store, &team.id, &draft, 1).await.unwrap();
        let builtins: Vec<BuiltinSkill> = skills
            .iter()
            .map(|(n, s)| (*n, &*Box::leak(s.clone().into_boxed_str())))
            .collect();
        let catalog = SkillCatalog::load_from(&builtins, None);
        let all: Vec<_> = catalog.skills().cloned().collect();
        let records = crate::repo::SkillRepository::sync_skills(&store, &all, 1)
            .await
            .unwrap();
        (store, catalog, agent.id, records)
    }

    fn assign(records: &[crate::repo::SkillRecord], names: &[(&str, bool)]) -> Vec<AgentSkill> {
        names
            .iter()
            .map(|(n, enabled)| AgentSkill {
                skill_id: records.iter().find(|r| r.slug == *n).unwrap().id.clone(),
                enabled: *enabled,
            })
            .collect()
    }

    #[tokio::test]
    async fn skill_so_do_claude_num_agente_codex_e_ignorada_com_aviso() {
        let (store, catalog, agent, records) = setup(
            "codex",
            &[
                ("so-claude", md("so-claude", "targets: [claude]\n")),
                ("geral", md("geral", "")),
            ],
        )
        .await;
        store
            .set_agent_skills(
                &agent,
                &assign(&records, &[("so-claude", true), ("geral", true)]),
            )
            .await
            .unwrap();
        let resolved = resolve_agent_skills(&store, &catalog, &agent, "codex")
            .await
            .unwrap();
        assert_eq!(resolved.plan().active, ["geral"]);
        assert_eq!(resolved.ignored.len(), 1);
        assert_eq!(
            resolved.ignored[0].reason,
            IgnoreReason::Incompatible {
                adapter_id: "codex".into(),
                targets: vec!["claude".into()],
            }
        );
        assert_eq!(
            resolved.ignored[0].message,
            "skill so-claude ignorada: não roda em codex (só claude)"
        );
        assert_eq!(
            resolved.ignored[0].to_string(),
            "so-claude does not run on codex (only claude)"
        );
    }

    #[tokio::test]
    async fn ordena_por_prioridade_e_depois_pela_posicao_e_pula_desabilitadas() {
        let (store, catalog, agent, records) = setup(
            "claude",
            &[
                ("a", md("a", "priority: 50\n")),
                ("b", md("b", "priority: 10\n")),
                ("c", md("c", "priority: 50\n")),
                ("off", md("off", "priority: 0\n")),
            ],
        )
        .await;
        store
            .set_agent_skills(
                &agent,
                &assign(
                    &records,
                    &[("c", true), ("a", true), ("off", false), ("b", true)],
                ),
            )
            .await
            .unwrap();
        let plan = resolve_agent_skills(&store, &catalog, &agent, "claude")
            .await
            .unwrap()
            .plan();
        assert_eq!(plan.active, ["b", "c", "a"]);
        assert!(plan.ignored.is_empty());
    }

    #[tokio::test]
    async fn skill_fora_do_disco_e_ignorada_como_ausente() {
        let (store, _, agent, records) = setup("claude", &[("sumiu", md("sumiu", ""))]).await;
        store
            .set_agent_skills(&agent, &assign(&records, &[("sumiu", true)]))
            .await
            .unwrap();
        let empty = SkillCatalog::load_from(&[], None);
        let resolved = resolve_agent_skills(&store, &empty, &agent, "claude")
            .await
            .unwrap();
        assert!(resolved.active.is_empty());
        assert_eq!(resolved.ignored[0].reason, IgnoreReason::Missing);
    }

    #[tokio::test]
    async fn trabalho_em_equipe_atribuida_nao_entra_duas_vezes() {
        let (store, catalog, agent, records) = setup(
            "claude",
            &[
                (TEAMWORK_SKILL, md(TEAMWORK_SKILL, "")),
                ("outra", md("outra", "")),
            ],
        )
        .await;
        store
            .set_agent_skills(
                &agent,
                &assign(&records, &[(TEAMWORK_SKILL, true), ("outra", true)]),
            )
            .await
            .unwrap();
        let resolved = resolve_agent_skills(&store, &catalog, &agent, "claude")
            .await
            .unwrap();
        assert_eq!(resolved.plan().active, ["outra"]);
        assert!(resolved.ignored.is_empty());
    }
}
