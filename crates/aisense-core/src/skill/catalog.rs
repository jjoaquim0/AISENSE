//! Biblioteca de skills: embutidas + `~/.aisense/skills/<pasta>/SKILL.md` (`docs/06`).

use std::collections::BTreeMap;
use std::path::Path;

use super::model::{Skill, SkillProblem, SkillSource};
use super::parse::parse_skill;

pub const SKILL_FILE: &str = "SKILL.md";

/// Uma skill que acompanha o binário: pasta e conteúdo do `SKILL.md`.
pub type BuiltinSkill = (&'static str, &'static str);

/// Skills mantidas pelo projeto (`skills/` na raiz do repositório). As do v1 entram na
/// F04-07; o mecanismo de carga já as trata como qualquer outra.
pub const BUILTIN_SKILLS: &[BuiltinSkill] = &[];

/// Resultado de uma carga. Sempre existe: arquivo ruim vira `problem`, nunca erro que
/// impede o app de subir.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillCatalog {
    skills: BTreeMap<String, Skill>,
    problems: Vec<SkillProblem>,
}

impl SkillCatalog {
    /// Embutidas + pasta do usuário. Pasta inexistente é o caso comum, não erro.
    pub fn load(user_dir: &Path) -> Self {
        Self::load_from(BUILTIN_SKILLS, Some(user_dir))
    }

    pub fn load_from(builtins: &[BuiltinSkill], user_dir: Option<&Path>) -> Self {
        let mut catalog = Self::default();
        for (dir, source) in builtins {
            let path = format!("builtin:{dir}/{SKILL_FILE}");
            match parse_skill(source, &path, SkillSource::Builtin) {
                Ok(skill) => {
                    catalog.skills.insert(skill.name.clone(), skill);
                }
                Err(problem) => catalog.report(problem),
            }
        }
        if let Some(dir) = user_dir {
            catalog.load_user_dir(dir);
        }
        catalog
    }

    fn load_user_dir(&mut self, dir: &Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) => {
                self.report(SkillProblem {
                    path: dir.display().to_string(),
                    line: None,
                    message: format!("could not read the skills folder: {err}"),
                });
                return;
            }
        };
        // Uma pasta por skill; soltos na raiz (README, .DS_Store) não são skill.
        let mut folders: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|p| p.is_dir())
            .collect();
        // Ordem estável: com duas pastas de mesmo `name`, vence sempre a mesma.
        folders.sort();

        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for folder in folders {
            let file = folder.join(SKILL_FILE);
            let shown = file.display().to_string();
            let source = match std::fs::read_to_string(&file) {
                Ok(source) => source,
                // Pasta sem SKILL.md: rascunho ou arquivos de apoio, não é problema.
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => {
                    self.report(SkillProblem {
                        path: shown,
                        line: None,
                        message: format!("could not read the file: {err}"),
                    });
                    continue;
                }
            };
            let origin = SkillSource::User {
                dir: folder.display().to_string(),
            };
            match parse_skill(&source, &shown, origin) {
                Ok(skill) => {
                    if let Some(first) = seen.get(&skill.name) {
                        self.report(SkillProblem {
                            path: shown,
                            line: None,
                            message: format!(
                                "skill {:?} is already defined in {first}; this one was ignored",
                                skill.name
                            ),
                        });
                        continue;
                    }
                    seen.insert(skill.name.clone(), shown);
                    // Precedência do usuário: sobrescreve a embutida de mesmo nome.
                    self.skills.insert(skill.name.clone(), skill);
                }
                Err(problem) => self.report(problem),
            }
        }
    }

    fn report(&mut self, problem: SkillProblem) {
        tracing::warn!(%problem, "skill ignorada");
        self.problems.push(problem);
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Em ordem de nome.
    pub fn skills(&self) -> impl Iterator<Item = &Skill> {
        self.skills.values()
    }

    pub fn problems(&self) -> &[SkillProblem] {
        &self.problems
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn skill_md(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n# {name}\n\nFaça bem.\n")
    }

    fn write(dir: &Path, folder: &str, content: &str) {
        let folder = dir.join(folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(SKILL_FILE), content).unwrap();
    }

    #[test]
    fn embutidas_e_do_usuario_com_precedencia_do_usuario() {
        let builtin = skill_md("revisor", "Embutido.");
        let builtins: &[BuiltinSkill] = &[("revisor", Box::leak(builtin.into_boxed_str()))];
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "revisor", &skill_md("revisor", "Meu revisor."));
        write(dir.path(), "outra", &skill_md("outra", "Outra."));

        let catalog = SkillCatalog::load_from(builtins, Some(dir.path()));
        assert!(catalog.problems().is_empty(), "{:?}", catalog.problems());
        let names: Vec<_> = catalog.skills().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["outra", "revisor"]);
        let revisor = catalog.get("revisor").unwrap();
        assert_eq!(revisor.description, "Meu revisor.");
        assert!(matches!(revisor.source, SkillSource::User { .. }));
    }

    #[test]
    fn arquivo_ruim_vira_aviso_com_caminho_e_linha_e_o_resto_carrega() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "boa", &skill_md("boa", "Boa."));
        write(dir.path(), "ruim", "---\nname: ruim\n---\nsem descrição\n");
        std::fs::create_dir_all(dir.path().join("rascunho")).unwrap();
        std::fs::write(dir.path().join("README.md"), "solto").unwrap();

        let catalog = SkillCatalog::load_from(&[], Some(dir.path()));
        assert_eq!(catalog.skills().count(), 1);
        let [problem] = catalog.problems() else {
            panic!("um problema: {:?}", catalog.problems())
        };
        assert!(problem.path.ends_with("SKILL.md"), "{problem}");
        assert!(problem.path.contains("ruim"));
        assert!(problem.message.contains("description"));
    }

    #[test]
    fn nome_repetido_na_pasta_do_usuario_fica_com_a_primeira() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a", &skill_md("dup", "Primeira."));
        write(dir.path(), "b", &skill_md("dup", "Segunda."));
        let catalog = SkillCatalog::load_from(&[], Some(dir.path()));
        assert_eq!(catalog.get("dup").unwrap().description, "Primeira.");
        assert_eq!(catalog.problems().len(), 1);
    }

    #[test]
    fn pasta_inexistente_nao_e_problema() {
        let catalog = SkillCatalog::load(Path::new("/nao/existe/aisense-skills"));
        assert!(catalog.problems().is_empty());
    }
}
