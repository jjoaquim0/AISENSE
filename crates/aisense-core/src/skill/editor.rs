//! Editor e biblioteca de skills (F04-08, tela T7): validar um rascunho enquanto o
//! usuário digita, salvar em `~/.aisense/skills/<nome>/`, duplicar, excluir, importar e
//! exportar uma pasta, e dizer **quem** precisa reiniciar depois de uma edição.
//!
//! Tudo que escreve fica dentro da pasta de skills do usuário: um caminho vindo da UI que
//! aponte para fora dela é recusado, nunca seguido. As embutidas são só leitura — para
//! mudar uma, duplica-se (a cópia do usuário com o mesmo nome venceria a embutida).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog::{SkillCatalog, SKILL_FILE};
use super::materialize::{copy_tree, write_skill, MaterializeError};
use super::model::{Skill, SkillProblem, SkillSource};
use super::parse::{frontmatter_key_line, parse_skill, SKILL_NAME_MAX};
use crate::agent::AgentState;
use crate::ids::{AgentId, TeamId};
use crate::repo::{AgentRepository, RepoResult, SkillRepository, TeamRepository};

/// Teto do `BOOT.md` inteiro (`docs/06`, "Anatomia do BOOT.md"). Uma skill sozinha perto
/// disso já força o compositor a resumir as outras.
pub const SKILL_BOOT_LIMIT: usize = 12_000;
/// A partir daqui o contador do editor fica amarelo.
const NEAR_LIMIT_PERCENT: usize = 80;
/// Nome que o rascunho recebe nas mensagens: ainda não tem pasta.
const DRAFT_PATH: &str = SKILL_FILE;
const TMP_FILE: &str = ".SKILL.md.aisense-tmp";

/// O que o editor mostra a cada tecla: a skill como ficaria, o que impede salvar e o que
/// só merece atenção.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SkillCheck {
    /// `None` quando o frontmatter não valida.
    pub skill: Option<Skill>,
    /// Impedem salvar. Com `caminho:linha` quando dá para saber a linha.
    pub problems: Vec<SkillProblem>,
    /// Frases prontas em pt-BR; não impedem salvar.
    pub warnings: Vec<String>,
    /// Caracteres do corpo — o que pesa no `BOOT.md`.
    pub chars: usize,
    pub boot_limit: usize,
}

impl SkillCheck {
    pub fn is_valid(&self) -> bool {
        self.skill.is_some() && self.problems.is_empty()
    }
}

/// Uma skill aberta no editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct OpenedSkill {
    /// O `SKILL.md` inteiro, como está no disco agora.
    pub source: String,
    /// Pasta da skill do usuário; `None` para a embutida (só leitura).
    pub dir: Option<String>,
    pub builtin: bool,
}

/// Um agente que tem a skill atribuída — a lista exata do aviso "precisam reiniciar".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SkillUser {
    pub agent_id: AgentId,
    pub handle: String,
    pub team_id: TeamId,
    pub team_name: String,
    /// Atribuída e ligada: entra no próximo boot.
    pub enabled: bool,
    /// Tem processo vivo. Ligada **e** rodando = precisa reiniciar para ver a mudança.
    pub running: bool,
}

impl SkillUser {
    pub fn needs_restart(&self) -> bool {
        self.enabled && self.running
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SkillEditError {
    #[error("the skill has problems: {}", join_problems(.0))]
    Invalid(Vec<SkillProblem>),
    #[error("a folder named {0:?} already exists")]
    FolderExists(String),
    #[error("{0} is not inside the skills library")]
    OutsideLibrary(String),
    #[error("skill {0:?} is built in and read-only")]
    Builtin(String),
    #[error("skill {0:?} was not found")]
    NotFound(String),
    #[error("could not access {path}: {source}")]
    Io { path: String, source: io::Error },
}

fn join_problems(problems: &[SkillProblem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

impl SkillEditError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "skill_invalid",
            Self::FolderExists(_) => "skill_exists",
            Self::OutsideLibrary(_) => "skill_outside_library",
            Self::Builtin(_) => "skill_builtin",
            Self::NotFound(_) => "skill_not_found",
            Self::Io { .. } => "skill_io",
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Self::FolderExists(_) => Some("Escolha outro nome para a skill.".into()),
            Self::Builtin(_) => Some("Duplique a skill para ter uma cópia editável.".into()),
            Self::OutsideLibrary(_) => {
                Some("Só dá para mexer em skills da sua pasta de skills.".into())
            }
            _ => None,
        }
    }

    pub fn to_command_error(&self) -> crate::CommandError {
        crate::CommandError::new(self.code(), self.to_string(), self.hint())
    }
}

impl From<MaterializeError> for SkillEditError {
    fn from(error: MaterializeError) -> Self {
        match error {
            MaterializeError::Io { path, source } => Self::Io { path, source },
        }
    }
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> SkillEditError + '_ {
    move |source| SkillEditError::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Valida um rascunho contra a biblioteca atual. `editing` é a pasta da skill aberta
/// (`None` para uma nova); `known_runtime` diz se um `adapter_id` existe.
pub fn check_skill(
    source: &str,
    catalog: &SkillCatalog,
    known_runtime: impl Fn(&str) -> bool,
    editing: Option<&str>,
) -> SkillCheck {
    let origin = SkillSource::User {
        dir: editing.unwrap_or_default().to_owned(),
    };
    let skill = match parse_skill(source, DRAFT_PATH, origin) {
        Ok(skill) => skill,
        Err(problem) => {
            return SkillCheck {
                skill: None,
                problems: vec![problem],
                warnings: Vec::new(),
                chars: source.chars().count(),
                boot_limit: SKILL_BOOT_LIMIT,
            }
        }
    };
    let mut problems = Vec::new();
    let mut warnings = Vec::new();
    let at = |key: &str, message: String| SkillProblem {
        path: DRAFT_PATH.to_owned(),
        line: frontmatter_key_line(source, key),
        message,
    };

    // Renomear na edição soltaria as atribuições: o banco casa a skill pelo nome.
    let original = editing.and_then(|dir| user_skill_in(catalog, dir));
    if let Some(original) = original {
        if original.name != skill.name {
            problems.push(at(
                "name",
                format!(
                    "o nome não muda na edição (era {:?}): os agentes encontram a skill por ele. \
                     Para outro nome, use Duplicar",
                    original.name
                ),
            ));
        }
    }

    match catalog.get(&skill.name).map(|existing| &existing.source) {
        Some(SkillSource::User { dir }) if Some(dir.as_str()) != editing => problems.push(at(
            "name",
            format!("já existe uma skill chamada {:?} em {dir}", skill.name),
        )),
        Some(SkillSource::Builtin) => warnings.push(format!(
            "substitui a skill embutida {:?} enquanto esta existir",
            skill.name
        )),
        _ => {}
    }

    let unknown: Vec<_> = skill
        .targets
        .iter()
        .filter(|t| !known_runtime(t))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        problems.push(at(
            "targets",
            format!(
                "runtime desconhecido em targets: {} — confira o id do adaptador",
                unknown.join(", ")
            ),
        ));
    }

    let chars = skill.body.chars().count();
    if chars > SKILL_BOOT_LIMIT {
        warnings.push(format!(
            "o corpo tem {chars} caracteres, mais que os {SKILL_BOOT_LIMIT} do BOOT.md inteiro: \
             o agente vai receber só o resumo e ler o resto sob demanda"
        ));
    } else if chars * 100 >= SKILL_BOOT_LIMIT * NEAR_LIMIT_PERCENT {
        warnings.push(format!(
            "o corpo tem {chars} caracteres, perto do limite de {SKILL_BOOT_LIMIT} do BOOT.md \
             inteiro: com outras skills, o compositor passa a resumir"
        ));
    }

    SkillCheck {
        skill: Some(skill),
        problems,
        warnings,
        chars,
        boot_limit: SKILL_BOOT_LIMIT,
    }
}

fn user_skill_in<'a>(catalog: &'a SkillCatalog, dir: &str) -> Option<&'a Skill> {
    catalog
        .skills()
        .find(|s| matches!(&s.source, SkillSource::User { dir: d } if d == dir))
}

/// Salva o rascunho. Nova: cria `<user_dir>/<nome>/`, recusando pasta que já existe.
/// Edição: reescreve o `SKILL.md` da pasta aberta, que tem de estar na biblioteca.
/// A escrita é atômica (arquivo temporário + rename): o hot-reload nunca lê meio arquivo.
pub fn save_skill(
    user_dir: &Path,
    catalog: &SkillCatalog,
    known_runtime: impl Fn(&str) -> bool,
    source: &str,
    editing: Option<&str>,
) -> Result<Skill, SkillEditError> {
    let check = check_skill(source, catalog, known_runtime, editing);
    let skill = match check.skill {
        Some(skill) if check.problems.is_empty() => skill,
        _ => return Err(SkillEditError::Invalid(check.problems)),
    };
    let dir = match editing {
        Some(dir) => inside_library(user_dir, Path::new(dir))?,
        None => {
            let dir = user_dir.join(&skill.name);
            if dir.exists() {
                return Err(SkillEditError::FolderExists(skill.name));
            }
            dir
        }
    };
    fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let tmp = dir.join(TMP_FILE);
    fs::write(&tmp, source).map_err(io_err(&tmp))?;
    let file = dir.join(SKILL_FILE);
    fs::rename(&tmp, &file).map_err(io_err(&file))?;
    Ok(Skill {
        source: SkillSource::User {
            dir: dir.display().to_string(),
        },
        ..skill
    })
}

/// O `SKILL.md` de uma skill da biblioteca, lido agora (a do usuário pode ter mudado no
/// disco depois da última carga).
pub fn open_skill(catalog: &SkillCatalog, name: &str) -> Result<OpenedSkill, SkillEditError> {
    let skill = catalog
        .get(name)
        .ok_or_else(|| SkillEditError::NotFound(name.to_owned()))?;
    match &skill.source {
        SkillSource::Builtin => Ok(OpenedSkill {
            source: skill.raw.clone(),
            dir: None,
            builtin: true,
        }),
        SkillSource::User { dir } => {
            let file = Path::new(dir).join(SKILL_FILE);
            let source = fs::read_to_string(&file).unwrap_or_else(|_| skill.raw.clone());
            Ok(OpenedSkill {
                source,
                dir: Some(dir.clone()),
                builtin: false,
            })
        }
    }
}

/// Um `SKILL.md` que não carregou (a lista de problemas da biblioteca), para consertar.
pub fn open_skill_file(user_dir: &Path, path: &Path) -> Result<OpenedSkill, SkillEditError> {
    let file = inside_library(user_dir, path)?;
    let source = fs::read_to_string(&file).map_err(io_err(&file))?;
    let dir = file
        .parent()
        .map(|p| p.display().to_string())
        .ok_or_else(|| SkillEditError::OutsideLibrary(path.display().to_string()))?;
    Ok(OpenedSkill {
        source,
        dir: Some(dir),
        builtin: false,
    })
}

/// O rascunho de uma cópia: o mesmo `SKILL.md` com um nome livre (`<nome>-copia`,
/// `<nome>-copia-2`...). Ainda não salva nada — o editor abre a cópia como skill nova.
pub fn duplicate_source(
    user_dir: &Path,
    catalog: &SkillCatalog,
    name: &str,
) -> Result<String, SkillEditError> {
    let opened = open_skill(catalog, name)?;
    let taken =
        |candidate: &str| catalog.get(candidate).is_some() || user_dir.join(candidate).exists();
    let base: String = name
        .chars()
        .take(SKILL_NAME_MAX - "-copia-99".len())
        .collect();
    let mut candidate = format!("{base}-copia");
    let mut n = 2;
    while taken(&candidate) {
        candidate = format!("{base}-copia-{n}");
        n += 1;
    }
    Ok(rename_frontmatter(&opened.source, &candidate))
}

/// Troca o valor de `name:` no frontmatter, preservando o resto do arquivo como está.
fn rename_frontmatter(source: &str, name: &str) -> String {
    let mut out = String::with_capacity(source.len() + name.len());
    let mut in_front = false;
    let mut done = false;
    for (i, line) in source.split_inclusive('\n').enumerate() {
        let bare = line.trim_start_matches('\u{feff}').trim_end();
        if bare == "---" {
            if i == 0 {
                in_front = true;
            } else if in_front {
                in_front = false;
            }
        } else if in_front && !done && line.starts_with("name:") {
            let ending = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.push_str("name: ");
            out.push_str(name);
            out.push_str(ending);
            done = true;
            continue;
        }
        out.push_str(line);
    }
    out
}

/// Apaga a pasta de uma skill do usuário. As atribuições ficam: a skill passa a aparecer
/// como "não está mais no disco" (F04-02), e os agentes são avisados no próximo início.
pub fn delete_skill(user_dir: &Path, dir: &str) -> Result<(), SkillEditError> {
    let dir = inside_library(user_dir, Path::new(dir))?;
    fs::remove_dir_all(&dir).map_err(io_err(&dir))
}

/// Copia uma pasta de skill (ou a pasta de um `SKILL.md` escolhido) para a biblioteca,
/// com os arquivos de apoio. Valida antes de copiar; nunca sobrescreve uma pasta.
pub fn import_skill(
    user_dir: &Path,
    catalog: &SkillCatalog,
    known_runtime: impl Fn(&str) -> bool,
    from: &Path,
) -> Result<Skill, SkillEditError> {
    let folder = if from.file_name().is_some_and(|n| n == SKILL_FILE) {
        from.parent().unwrap_or(from)
    } else {
        from
    };
    let file = folder.join(SKILL_FILE);
    let source = fs::read_to_string(&file).map_err(io_err(&file))?;
    let check = check_skill(&source, catalog, known_runtime, None);
    let skill = match check.skill {
        Some(skill) if check.problems.is_empty() => skill,
        _ => {
            let shown = file.display().to_string();
            let problems = check
                .problems
                .into_iter()
                .map(|p| SkillProblem {
                    path: shown.clone(),
                    ..p
                })
                .collect();
            return Err(SkillEditError::Invalid(problems));
        }
    };
    let dest = user_dir.join(&skill.name);
    if dest.exists() {
        return Err(SkillEditError::FolderExists(skill.name));
    }
    fs::create_dir_all(&dest).map_err(io_err(&dest))?;
    copy_tree(folder, &dest)?;
    Ok(Skill {
        source: SkillSource::User {
            dir: dest.display().to_string(),
        },
        ..skill
    })
}

/// Escreve a skill em `<to>/<nome>/` — o `SKILL.md` e, se for do usuário, os arquivos de
/// apoio. Nunca sobrescreve. Devolve a pasta criada.
pub fn export_skill(
    catalog: &SkillCatalog,
    name: &str,
    to: &Path,
) -> Result<PathBuf, SkillEditError> {
    let skill = catalog
        .get(name)
        .ok_or_else(|| SkillEditError::NotFound(name.to_owned()))?;
    let dest = to.join(&skill.name);
    if dest.exists() {
        return Err(SkillEditError::FolderExists(skill.name.clone()));
    }
    write_skill(skill, &dest)?;
    Ok(dest)
}

/// `path` resolvido, desde que esteja **dentro** da pasta de skills (e não seja ela).
fn inside_library(user_dir: &Path, path: &Path) -> Result<PathBuf, SkillEditError> {
    let outside = || SkillEditError::OutsideLibrary(path.display().to_string());
    let root = user_dir.canonicalize().map_err(|_| outside())?;
    let resolved = path.canonicalize().map_err(|_| outside())?;
    if resolved.starts_with(&root) && resolved != root {
        Ok(resolved)
    } else {
        Err(outside())
    }
}

/// Quem tem a skill atribuída, com equipe, se está ligada e se está rodando agora.
/// Ordem: primeiro quem precisa reiniciar, depois por equipe e endereço.
pub async fn skill_users<S>(
    store: &S,
    name: &str,
    state_of: impl Fn(&AgentId) -> AgentState,
) -> RepoResult<Vec<SkillUser>>
where
    S: SkillRepository + AgentRepository + TeamRepository,
{
    let Some(record) = store
        .list_skills()
        .await?
        .into_iter()
        .find(|r| r.slug == name)
    else {
        return Ok(Vec::new());
    };
    let mut users = Vec::new();
    for agent_id in store.skill_users(&record.id).await? {
        let Some(agent) = store.get_agent(&agent_id).await? else {
            continue;
        };
        let enabled = store
            .agent_skills(&agent_id)
            .await?
            .iter()
            .any(|s| s.skill_id == record.id && s.enabled);
        let team_name = store
            .get_team(&agent.team_id)
            .await?
            .map(|t| t.name)
            .unwrap_or_default();
        users.push(SkillUser {
            running: state_of(&agent_id).is_running(),
            agent_id,
            handle: agent.handle.as_str().to_owned(),
            team_id: agent.team_id,
            team_name,
            enabled,
        });
    }
    users.sort_by(|a, b| {
        b.needs_restart()
            .cmp(&a.needs_restart())
            .then_with(|| a.team_name.cmp(&b.team_name))
            .then_with(|| a.handle.cmp(&b.handle))
    });
    Ok(users)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::agent::{create_agent, AgentDraft};
    use crate::repo::{AgentSkill, InMemoryStore};
    use crate::skill::catalog::BuiltinSkill;
    use crate::team::{Team, TeamDraft};

    const BUILTIN: &str = "---\nname: embutida\ndescription: Vem com o app.\n---\n# Embutida\n";

    fn md(name: &str, extra: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: Faz {name}.\n{extra}---\n{body}\n")
    }

    fn known(id: &str) -> bool {
        matches!(id, "claude" | "codex" | "shell")
    }

    fn library(dir: &Path) -> SkillCatalog {
        let builtins: &[BuiltinSkill] = &[("embutida", BUILTIN)];
        SkillCatalog::load_from(builtins, Some(dir))
    }

    #[test]
    fn rascunho_valido_mostra_a_skill_e_o_tamanho_do_corpo() {
        let dir = tempfile::tempdir().unwrap();
        let check = check_skill(&md("nova", "", "corpo"), &library(dir.path()), known, None);
        assert!(check.is_valid(), "{check:?}");
        assert_eq!(check.chars, "corpo".len());
        assert_eq!(check.boot_limit, SKILL_BOOT_LIMIT);
        assert!(check.warnings.is_empty());
    }

    #[test]
    fn frontmatter_invalido_vira_problema_com_linha_nao_panico() {
        let dir = tempfile::tempdir().unwrap();
        let check = check_skill(
            "---\nname: x\ndescription: [\n---\ncorpo\n",
            &library(dir.path()),
            known,
            None,
        );
        assert!(check.skill.is_none());
        assert_eq!(check.problems.len(), 1);
        assert!(check.problems[0].line.is_some(), "{:?}", check.problems);
    }

    #[test]
    fn nome_repetido_e_targets_inexistente_apontam_a_linha() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = library(dir.path());
        save_skill(dir.path(), &catalog, known, &md("dup", "", "a"), None).unwrap();
        let catalog = library(dir.path());

        let draft = md("dup", "targets: [claude, gpt-9]\n", "b");
        let check = check_skill(&draft, &catalog, known, None);
        assert!(!check.is_valid());
        let lines: Vec<_> = check.problems.iter().map(|p| p.line).collect();
        assert_eq!(lines, [Some(2), Some(4)], "{:?}", check.problems);
        assert!(check.problems[0].message.contains("já existe"));
        assert!(check.problems[1].message.contains("gpt-9"));
    }

    #[test]
    fn mesmo_nome_de_embutida_so_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let check = check_skill(&md("embutida", "", "x"), &library(dir.path()), known, None);
        assert!(check.is_valid());
        assert_eq!(check.warnings.len(), 1);
    }

    #[test]
    fn corpo_perto_e_acima_do_limite_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = library(dir.path());
        let near = "a".repeat(SKILL_BOOT_LIMIT * 9 / 10);
        let check = check_skill(&md("perto", "", &near), &catalog, known, None);
        assert!(check.warnings[0].contains("perto do limite"));
        let over = "a".repeat(SKILL_BOOT_LIMIT + 1);
        let check = check_skill(&md("acima", "", &over), &catalog, known, None);
        assert!(check.warnings[0].contains("só o resumo"));
    }

    #[test]
    fn salvar_cria_edita_e_recusa_renomear_e_pasta_existente() {
        let dir = tempfile::tempdir().unwrap();
        let saved = save_skill(
            dir.path(),
            &library(dir.path()),
            known,
            &md("revisor", "", "v1"),
            None,
        )
        .unwrap();
        let SkillSource::User { dir: skill_dir } = &saved.source else {
            panic!("do usuário")
        };
        let catalog = library(dir.path());
        assert_eq!(catalog.get("revisor").unwrap().body, "v1");

        // De novo como nova: a pasta existe.
        let again = save_skill(dir.path(), &catalog, known, &md("revisor", "", "x"), None);
        assert!(matches!(again, Err(SkillEditError::Invalid(_))));

        // Edição mantém a pasta e troca o conteúdo.
        save_skill(
            dir.path(),
            &catalog,
            known,
            &md("revisor", "", "v2"),
            Some(skill_dir),
        )
        .unwrap();
        let catalog = library(dir.path());
        assert_eq!(catalog.get("revisor").unwrap().body, "v2");
        assert!(!Path::new(skill_dir).join(TMP_FILE).exists());

        // Renomear na edição soltaria as atribuições.
        let renamed = save_skill(
            dir.path(),
            &catalog,
            known,
            &md("outro", "", "v3"),
            Some(skill_dir),
        );
        let Err(SkillEditError::Invalid(problems)) = renamed else {
            panic!("recusado: {renamed:?}")
        };
        assert!(problems[0].message.contains("Duplicar"));
    }

    #[test]
    fn nada_e_escrito_nem_apagado_fora_da_biblioteca() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join(SKILL_FILE), md("fora", "", "x")).unwrap();
        let catalog = library(dir.path());
        let path = outside.path().display().to_string();

        let saved = save_skill(
            dir.path(),
            &catalog,
            known,
            &md("fora", "", "y"),
            Some(&path),
        );
        assert!(matches!(saved, Err(SkillEditError::OutsideLibrary(_))));
        assert!(matches!(
            delete_skill(dir.path(), &path),
            Err(SkillEditError::OutsideLibrary(_))
        ));
        let escape = dir.path().join("..").display().to_string();
        assert!(matches!(
            delete_skill(dir.path(), &escape),
            Err(SkillEditError::OutsideLibrary(_))
        ));
        assert!(matches!(
            delete_skill(dir.path(), &dir.path().display().to_string()),
            Err(SkillEditError::OutsideLibrary(_))
        ));
        assert!(outside.path().join(SKILL_FILE).exists());
    }

    #[test]
    fn duplicar_gera_nome_livre_e_preserva_o_resto() {
        let dir = tempfile::tempdir().unwrap();
        let source = "---\r\nname: embutida\r\ndescription: Vem.\r\n---\r\n# Corpo\r\n";
        let builtins: &[BuiltinSkill] = &[("embutida", source)];
        std::fs::create_dir_all(dir.path().join("embutida-copia")).unwrap();
        let catalog = SkillCatalog::load_from(builtins, Some(dir.path()));

        let copy = duplicate_source(dir.path(), &catalog, "embutida").unwrap();
        assert_eq!(
            copy,
            "---\r\nname: embutida-copia-2\r\ndescription: Vem.\r\n---\r\n# Corpo\r\n"
        );
        assert!(check_skill(&copy, &catalog, known, None).is_valid());
    }

    #[test]
    fn embutida_abre_so_leitura_e_do_usuario_le_o_disco() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = library(dir.path());
        let opened = open_skill(&catalog, "embutida").unwrap();
        assert!(opened.builtin && opened.dir.is_none());
        assert_eq!(opened.source, BUILTIN);

        save_skill(dir.path(), &catalog, known, &md("minha", "", "v1"), None).unwrap();
        let catalog = library(dir.path());
        let file = dir.path().join("minha").join(SKILL_FILE);
        std::fs::write(&file, md("minha", "", "mudou no disco")).unwrap();
        let opened = open_skill(&catalog, "minha").unwrap();
        assert!(opened.source.contains("mudou no disco"));
        assert!(matches!(
            open_skill(&catalog, "nao-existe"),
            Err(SkillEditError::NotFound(_))
        ));

        // Arquivo quebrado abre pelo caminho, para ser consertado.
        std::fs::create_dir_all(dir.path().join("ruim")).unwrap();
        let broken = dir.path().join("ruim").join(SKILL_FILE);
        std::fs::write(&broken, "sem frontmatter").unwrap();
        let opened = open_skill_file(dir.path(), &broken).unwrap();
        assert_eq!(opened.source, "sem frontmatter");
        assert!(opened.dir.unwrap().ends_with("ruim"));
    }

    #[test]
    fn importar_e_exportar_levam_os_arquivos_de_apoio() {
        let lib = tempfile::tempdir().unwrap();
        let from = tempfile::tempdir().unwrap();
        let pkg = from.path().join("pacote");
        std::fs::create_dir_all(pkg.join("references")).unwrap();
        std::fs::write(pkg.join(SKILL_FILE), md("importada", "", "corpo")).unwrap();
        std::fs::write(pkg.join("references").join("lista.md"), "- item").unwrap();

        // Escolher o próprio SKILL.md também vale.
        let skill = import_skill(
            lib.path(),
            &library(lib.path()),
            known,
            &pkg.join(SKILL_FILE),
        )
        .unwrap();
        assert_eq!(skill.name, "importada");
        let copied = lib.path().join("importada");
        assert!(copied.join("references").join("lista.md").exists());
        let again = import_skill(lib.path(), &library(lib.path()), known, &pkg);
        assert!(again.is_err(), "já está na biblioteca");

        let out = tempfile::tempdir().unwrap();
        let catalog = library(lib.path());
        let dest = export_skill(&catalog, "importada", out.path()).unwrap();
        assert!(dest.join("references").join("lista.md").exists());
        assert!(matches!(
            export_skill(&catalog, "importada", out.path()),
            Err(SkillEditError::FolderExists(_))
        ));
        let dest = export_skill(&catalog, "embutida", out.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join(SKILL_FILE)).unwrap(),
            BUILTIN
        );
    }

    #[test]
    fn importar_skill_invalida_aponta_o_arquivo_de_origem() {
        let lib = tempfile::tempdir().unwrap();
        let from = tempfile::tempdir().unwrap();
        std::fs::write(from.path().join(SKILL_FILE), "---\nname: x\n---\ncorpo\n").unwrap();
        let Err(SkillEditError::Invalid(problems)) =
            import_skill(lib.path(), &library(lib.path()), known, from.path())
        else {
            panic!("inválida")
        };
        assert!(problems[0]
            .path
            .starts_with(&from.path().display().to_string()));
        assert_eq!(std::fs::read_dir(lib.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn quem_usa_a_skill_e_quem_precisa_reiniciar() {
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
        let mut agents = Vec::new();
        for handle in ["ana", "bia", "caio"] {
            let draft = AgentDraft {
                handle: handle.into(),
                name: handle.into(),
                adapter_id: "claude".into(),
                ..AgentDraft::default()
            };
            agents.push(create_agent(&store, &team.id, &draft, 1).await.unwrap());
        }
        let catalog = SkillCatalog::load_from(&[("alvo", &*md("alvo", "", "x").leak())], None);
        let records = store
            .sync_skills(&catalog.skills().cloned().collect::<Vec<_>>(), 1)
            .await
            .unwrap();
        let id = records[0].id.clone();
        // ana: ligada e parada · bia: ligada e rodando · caio: desligada e rodando.
        for (agent, enabled) in agents.iter().zip([true, true, false]) {
            store
                .set_agent_skills(
                    &agent.id,
                    &[AgentSkill {
                        skill_id: id.clone(),
                        enabled,
                    }],
                )
                .await
                .unwrap();
        }
        let running = [agents[1].id.clone(), agents[2].id.clone()];
        let users = skill_users(&store, "alvo", |id| {
            if running.contains(id) {
                AgentState::Idle
            } else {
                AgentState::Stopped
            }
        })
        .await
        .unwrap();
        let summary: Vec<_> = users
            .iter()
            .map(|u| (u.handle.as_str(), u.needs_restart(), u.team_name.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                ("bia", true, "Squad"),
                ("ana", false, "Squad"),
                ("caio", false, "Squad")
            ]
        );
        assert!(skill_users(&store, "ninguem", |_| AgentState::Idle)
            .await
            .unwrap()
            .is_empty());
    }
}
