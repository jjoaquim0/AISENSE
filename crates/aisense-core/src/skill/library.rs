//! A biblioteca de skills como a UI a vê: o catálogo do disco junto com a identidade e
//! o uso guardados no banco (F04-02).

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog::SkillCatalog;
use super::model::{SkillInject, SkillProblem, SkillSource};
use crate::ids::{AgentId, SkillId};
use crate::repo::{RepoResult, SkillRepository};
use crate::time::Millis;

/// O catálogo atual, trocado inteiro a cada recarga do disco.
#[derive(Default)]
pub struct SkillLibrary {
    catalog: RwLock<Arc<SkillCatalog>>,
}

impl SkillLibrary {
    pub fn new(catalog: SkillCatalog) -> Self {
        Self {
            catalog: RwLock::new(Arc::new(catalog)),
        }
    }

    pub fn catalog(&self) -> Arc<SkillCatalog> {
        Arc::clone(&self.catalog.read().unwrap_or_else(|p| p.into_inner()))
    }

    pub fn set_catalog(&self, catalog: SkillCatalog) {
        *self.catalog.write().unwrap_or_else(|p| p.into_inner()) = Arc::new(catalog);
    }

    /// Grava no banco o que está no disco agora.
    pub async fn sync<S: SkillRepository>(&self, store: &S, now: Millis) -> RepoResult<()> {
        let catalog = self.catalog();
        let skills: Vec<_> = catalog.skills().cloned().collect();
        store.sync_skills(&skills, now).await.map(|_| ())
    }

    /// Todas as skills conhecidas, com quem usa cada uma, e os arquivos que não carregaram.
    pub async fn view<S: SkillRepository>(&self, store: &S) -> RepoResult<SkillLibraryView> {
        let catalog = self.catalog();
        let mut skills = Vec::new();
        for record in store.list_skills().await? {
            let users = store.skill_users(&record.id).await?;
            let entry = match catalog.get(&record.slug) {
                Some(skill) => SkillEntry {
                    id: record.id,
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    version: skill.version.clone(),
                    targets: skill.targets.clone(),
                    inject: skill.inject,
                    priority: skill.priority,
                    source: Some(skill.source.clone()),
                    chars: skill.body.chars().count(),
                    users,
                },
                // Sumiu do disco (apagada ou com erro): continua listada para as
                // atribuições não desaparecerem em silêncio.
                None => SkillEntry {
                    id: record.id,
                    name: record.slug,
                    description: record.description,
                    version: record.version,
                    targets: record.targets,
                    inject: SkillInject::default(),
                    priority: 0,
                    source: None,
                    chars: 0,
                    users,
                },
            };
            skills.push(entry);
        }
        Ok(SkillLibraryView {
            skills,
            problems: catalog.problems().to_vec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SkillLibraryView {
    /// Por nome.
    pub skills: Vec<SkillEntry>,
    pub problems: Vec<SkillProblem>,
}

/// Uma skill na biblioteca, sem o corpo (o editor pede à parte).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SkillEntry {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub targets: Vec<String>,
    pub inject: SkillInject,
    pub priority: u8,
    /// `None` quando o `SKILL.md` não está mais no disco (ou não carrega).
    pub source: Option<SkillSource>,
    /// Tamanho do corpo em caracteres — pesa no orçamento do `BOOT.md`.
    pub chars: usize,
    /// Agentes que têm a skill atribuída (habilitada ou não).
    pub users: Vec<AgentId>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::agent::{create_agent, AgentDraft};
    use crate::repo::{AgentSkill, InMemoryStore, TeamRepository};
    use crate::skill::catalog::BuiltinSkill;
    use crate::team::{Team, TeamDraft};

    const A: &str = "---\nname: alfa\ndescription: Faz alfa.\n---\n# Alfa\n\ncorpo\n";
    const B: &str = "---\nname: beta\ndescription: Faz beta.\n---\ncorpo\n";

    #[tokio::test]
    async fn skill_que_some_do_disco_continua_listada_com_quem_a_usa() {
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
            adapter_id: "claude".into(),
            ..AgentDraft::default()
        };
        let agent = create_agent(&store, &team.id, &draft, 1).await.unwrap();

        let both: &[BuiltinSkill] = &[("alfa", A), ("beta", B)];
        let library = SkillLibrary::new(SkillCatalog::load_from(both, None));
        library.sync(&store, 1).await.unwrap();
        let view = library.view(&store).await.unwrap();
        assert_eq!(view.skills.len(), 2);
        let alfa = view
            .skills
            .iter()
            .find(|s| s.name == "alfa")
            .unwrap()
            .clone();
        assert_eq!(alfa.chars, "# Alfa\n\ncorpo".chars().count());
        assert!(alfa.source.is_some());

        store
            .set_agent_skills(
                &agent.id,
                &[AgentSkill {
                    skill_id: alfa.id.clone(),
                    enabled: true,
                }],
            )
            .await
            .unwrap();
        library.set_catalog(SkillCatalog::load_from(&[("beta", B)], None));
        library.sync(&store, 2).await.unwrap();
        let view = library.view(&store).await.unwrap();
        let alfa = view.skills.iter().find(|s| s.name == "alfa").unwrap();
        assert!(alfa.source.is_none(), "fora do disco");
        assert_eq!(alfa.users, vec![agent.id]);
    }
}
