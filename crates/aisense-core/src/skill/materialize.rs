//! Materialização (F04-04): põe no diretório de trabalho o que o agente precisa ler ao
//! nascer — as skills resolvidas e a identidade dele (`docs/02`, "Persistência e layout").
//!
//! ```text
//! <workdir>/.aisense/
//! ├── .gitignore                  gerado uma vez: ignora tudo menos notes/
//! ├── agents/<handle>/
//! │   ├── agent.json              identidade do agente
//! │   └── skills/<nome>/SKILL.md  skills resolvidas (com os arquivos de apoio)
//! └── native-skills.json          quem é dono de cada pasta em `skills.dir` do runtime
//! ```
//!
//! **Uma pasta por agente**, não `.aisense/agent.json` na raiz: no modo compartilhado
//! (o padrão) vários agentes trabalham no mesmo diretório, e um sobrescreveria o outro.
//!
//! Quando o adaptador declara `skills.dir` (ex.: `.claude/skills`), as skills também vão
//! para lá, onde o runtime as acha sozinho. Essa pasta é do usuário: **nunca** se
//! sobrescreve uma skill que não foi o AISENSE que escreveu, e só se apaga o que o
//! manifesto diz que é nosso. Num diretório compartilhado, a pasta nativa é uma só para
//! todos os agentes daquele runtime; o manifesto guarda os donos de cada skill e ela só
//! sai quando o último dono deixa de usá-la.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::catalog::SKILL_FILE;
use super::model::{Skill, SkillSource};
use crate::adapter::Adapter;
use crate::agent::Agent;
use crate::team::Team;
use crate::time::Millis;

pub const AISENSE_DIR: &str = ".aisense";
const AGENTS_DIR: &str = "agents";
const SKILLS_DIR: &str = "skills";
const AGENT_FILE: &str = "agent.json";
const NATIVE_MANIFEST: &str = "native-skills.json";
const GITIGNORE: &str = "# Gerado pelo AISENSE. Tudo aqui é regenerado a cada início de agente,\n\
# menos as notas da equipe (docs/15), que são suas e devem ir para o git.\n\
/*\n!/.gitignore\n!/notes/\n";

/// Um start por vez escreve em `.aisense/`: o manifesto da pasta nativa é compartilhado
/// entre os agentes do mesmo diretório, e ler-mudar-gravar em paralelo perderia donos.
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, thiserror::Error)]
pub enum MaterializeError {
    #[error("could not write {path}: {source}")]
    Io { path: String, source: io::Error },
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> MaterializeError + '_ {
    move |source| MaterializeError::Io {
        path: path.display().to_string(),
        source,
    }
}

/// O que foi escrito, para o compositor do `BOOT.md` (F04-05) e para a UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Materialized {
    /// `<workdir>/.aisense/agents/<handle>/`.
    pub agent_dir: PathBuf,
    /// Ressalvas que não impedem o start (skill nativa que já existia e não é nossa).
    pub warnings: Vec<String>,
}

/// A identidade que o agente lê em `agent.json`. Sem segredo nenhum: o token do
/// barramento vai só pelo ambiente do processo (regra R3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub id: String,
    pub handle: String,
    pub name: String,
    pub role: String,
    pub runtime: String,
    pub team: TeamCard,
    /// Na ordem de injeção.
    pub skills: Vec<SkillCard>,
    pub generated_at: Millis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamCard {
    pub id: String,
    pub name: String,
    pub mission: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillCard {
    pub name: String,
    pub version: String,
    /// Relativo ao diretório de trabalho.
    pub path: String,
}

/// Dono de cada pasta que o AISENSE escreveu em `skills.dir`: `pasta → handles`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct NativeManifest {
    #[serde(default)]
    dirs: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
}

pub struct MaterializeRequest<'a> {
    pub workdir: &'a Path,
    pub agent: &'a Agent,
    pub team: &'a Team,
    pub adapter: &'a Adapter,
    /// Já resolvidas (F04-03), na ordem de injeção.
    pub skills: &'a [Skill],
    pub now: Millis,
}

/// Escreve tudo e limpa o que sobrou do boot anterior. Bloqueia (disco): rode fora das
/// threads do runtime assíncrono.
pub fn materialize(req: &MaterializeRequest<'_>) -> Result<Materialized, MaterializeError> {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let root = req.workdir.join(AISENSE_DIR);
    fs::create_dir_all(&root).map_err(io_err(&root))?;
    let gitignore = root.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, GITIGNORE).map_err(io_err(&gitignore))?;
    }

    let handle = req.agent.handle.as_str();
    let agents = root.join(AGENTS_DIR);
    let old_handles = remove_stale_agent_dirs(&agents, req.agent.id.as_str(), handle)?;
    let agent_dir = agents.join(handle);

    // Recomeça as skills do agente do zero: o que não está na lista nova não fica.
    let skills_dir = agent_dir.join(SKILLS_DIR);
    remove_dir_if_exists(&skills_dir)?;
    fs::create_dir_all(&skills_dir).map_err(io_err(&skills_dir))?;
    for skill in req.skills {
        write_skill(skill, &skills_dir.join(&skill.name))?;
    }

    let card = AgentCard {
        id: req.agent.id.to_string(),
        handle: handle.to_owned(),
        name: req.agent.name.clone(),
        role: req.agent.role.clone(),
        runtime: req.adapter.id.clone(),
        team: TeamCard {
            id: req.team.id.to_string(),
            name: req.team.name.clone(),
            mission: req.team.mission.clone(),
        },
        skills: req
            .skills
            .iter()
            .map(|s| SkillCard {
                name: s.name.clone(),
                version: s.version.clone(),
                path: format!(
                    "{AISENSE_DIR}/{AGENTS_DIR}/{handle}/{SKILLS_DIR}/{}/{SKILL_FILE}",
                    s.name
                ),
            })
            .collect(),
        generated_at: req.now,
    };
    let card_path = agent_dir.join(AGENT_FILE);
    let json = serde_json::to_string_pretty(&card).map_err(|e| MaterializeError::Io {
        path: card_path.display().to_string(),
        source: io::Error::other(e),
    })?;
    fs::write(&card_path, json + "\n").map_err(io_err(&card_path))?;

    let mut warnings = Vec::new();
    if let Some(target) = &req.adapter.skills {
        sync_native(req, &root, &target.dir, handle, &old_handles, &mut warnings)?;
    }
    Ok(Materialized {
        agent_dir,
        warnings,
    })
}

/// Pastas `agents/<outro-handle>` do mesmo agente (renomeado desde o último boot).
/// Devolve os handles antigos, para soltar também a posse deles na pasta nativa.
fn remove_stale_agent_dirs(
    agents: &Path,
    agent_id: &str,
    handle: &str,
) -> Result<Vec<String>, MaterializeError> {
    let mut old = Vec::new();
    let Ok(entries) = fs::read_dir(agents) else {
        return Ok(old);
    };
    for entry in entries.filter_map(Result::ok) {
        let dir = entry.path();
        if dir.file_name().is_some_and(|n| n == handle) || !dir.is_dir() {
            continue;
        }
        let card = fs::read_to_string(dir.join(AGENT_FILE))
            .ok()
            .and_then(|json| serde_json::from_str::<AgentCard>(&json).ok());
        if let Some(card) = card.filter(|c| c.id == agent_id) {
            remove_dir_if_exists(&dir)?;
            old.push(card.handle);
        }
    }
    Ok(old)
}

/// Embutida: o `SKILL.md`. Do usuário: a pasta inteira (referências, scripts), sem links
/// simbólicos nem `.git` — copiar um link poderia puxar arquivo de fora da skill.
pub(super) fn write_skill(skill: &Skill, dest: &Path) -> Result<(), MaterializeError> {
    fs::create_dir_all(dest).map_err(io_err(dest))?;
    if let SkillSource::User { dir } = &skill.source {
        copy_tree(Path::new(dir), dest)?;
    }
    // O `SKILL.md` sempre do que foi lido e validado, não do disco de agora (pode ter
    // mudado no meio); para a embutida, é a única fonte.
    let file = dest.join(SKILL_FILE);
    fs::write(&file, &skill.raw).map_err(io_err(&file))
}

pub(super) fn copy_tree(from: &Path, to: &Path) -> Result<(), MaterializeError> {
    let entries = fs::read_dir(from).map_err(io_err(from))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        if kind.is_symlink() || name == ".git" {
            continue;
        }
        let target = to.join(&name);
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(io_err(&target))?;
            copy_tree(&path, &target)?;
        } else if kind.is_file() {
            fs::copy(&path, &target).map_err(io_err(&target))?;
        }
    }
    Ok(())
}

/// `skills.dir` do adaptador, relativo ao diretório de trabalho e sem sair dele.
fn native_root(workdir: &Path, dir: &str) -> Option<PathBuf> {
    let rel = Path::new(dir);
    let inside = rel
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    inside.then(|| workdir.join(rel))
}

fn sync_native(
    req: &MaterializeRequest<'_>,
    root: &Path,
    dir: &str,
    handle: &str,
    old_handles: &[String],
    warnings: &mut Vec<String>,
) -> Result<(), MaterializeError> {
    let Some(native) = native_root(req.workdir, dir) else {
        warnings.push(format!(
            "o runtime pede skills em {dir:?}, fora do diretório de trabalho; ignorado"
        ));
        return Ok(());
    };
    let manifest_path = root.join(NATIVE_MANIFEST);
    let mut manifest: NativeManifest = fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let owned = manifest.dirs.entry(dir.to_owned()).or_default();

    // Solta o que este agente não usa mais; pasta sem dono sai do disco.
    let wanted: BTreeSet<&str> = req.skills.iter().map(|s| s.name.as_str()).collect();
    let mut orphaned = Vec::new();
    for (name, owners) in owned.iter_mut() {
        for old in old_handles {
            owners.remove(old);
        }
        if !wanted.contains(name.as_str()) {
            owners.remove(handle);
        }
        if owners.is_empty() {
            orphaned.push(name.clone());
        }
    }
    for name in orphaned {
        owned.remove(&name);
        remove_dir_if_exists(&native.join(&name))?;
    }

    for skill in req.skills {
        let dest = native.join(&skill.name);
        if dest.exists() && !owned.contains_key(&skill.name) {
            warnings.push(format!(
                "{} já existe e não foi o AISENSE que criou; a skill {} não foi copiada para lá",
                dest.display(),
                skill.name
            ));
            continue;
        }
        remove_dir_if_exists(&dest)?;
        write_skill(skill, &dest)?;
        owned
            .entry(skill.name.clone())
            .or_default()
            .insert(handle.to_owned());
    }

    manifest.dirs.retain(|_, skills| !skills.is_empty());
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| MaterializeError::Io {
        path: manifest_path.display().to_string(),
        source: io::Error::other(e),
    })?;
    fs::write(&manifest_path, json + "\n").map_err(io_err(&manifest_path))
}

fn remove_dir_if_exists(dir: &Path) -> Result<(), MaterializeError> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err(dir)(e)),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::adapter::{AdapterCatalog, BUILTIN_ADAPTERS};
    use crate::agent::AgentDraft;
    use crate::skill::parse_skill;
    use crate::team::TeamDraft;

    struct Fixture {
        dir: tempfile::TempDir,
        team: Team,
        claude: Adapter,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let team = Team::create(
            &TeamDraft {
                name: "Squad".into(),
                mission: "Migrar a autenticação.".into(),
                workdir: dir.path().display().to_string(),
                ..TeamDraft::default()
            },
            1,
        )
        .unwrap();
        let catalog = AdapterCatalog::load_from(BUILTIN_ADAPTERS, None);
        let claude = catalog.get("claude").unwrap().clone();
        assert!(
            claude.skills.is_some(),
            "o claude embutido declara skills.dir"
        );
        Fixture { dir, team, claude }
    }

    fn agent(team: &Team, handle: &str, siblings: &[Agent]) -> Agent {
        Agent::create(
            team.id.clone(),
            &AgentDraft {
                handle: handle.into(),
                name: handle.to_uppercase(),
                role: "Revisa".into(),
                adapter_id: "claude".into(),
                ..AgentDraft::default()
            },
            siblings,
            1,
        )
        .unwrap()
    }

    fn builtin(name: &str) -> Skill {
        let md = format!("---\nname: {name}\ndescription: Faz {name}.\n---\n# {name}\n");
        parse_skill(&md, "t", SkillSource::Builtin).unwrap()
    }

    fn run(f: &Fixture, agent: &Agent, skills: &[Skill]) -> Materialized {
        materialize(&MaterializeRequest {
            workdir: f.dir.path(),
            agent,
            team: &f.team,
            adapter: &f.claude,
            skills,
            now: 42,
        })
        .unwrap()
    }

    /// Todos os arquivos sob `root`, relativos, em ordem.
    fn files(root: &Path) -> Vec<String> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).unwrap().filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, out);
                } else {
                    let rel = path.strip_prefix(base).unwrap();
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut out = Vec::new();
        if root.exists() {
            walk(root, root, &mut out);
        }
        out.sort();
        out
    }

    #[test]
    fn dois_inicios_com_skills_diferentes_nao_deixam_orfaos() {
        let f = fixture();
        let revisor = agent(&f.team, "revisor", &[]);
        run(&f, &revisor, &[builtin("alfa"), builtin("beta")]);
        run(&f, &revisor, &[builtin("beta"), builtin("gama")]);

        assert_eq!(
            files(&f.dir.path().join(".aisense")),
            [
                ".gitignore",
                "agents/revisor/agent.json",
                "agents/revisor/skills/beta/SKILL.md",
                "agents/revisor/skills/gama/SKILL.md",
                "native-skills.json",
            ]
        );
        assert_eq!(
            files(&f.dir.path().join(".claude/skills")),
            ["beta/SKILL.md", "gama/SKILL.md"]
        );
        // O SKILL.md é o original, frontmatter incluso.
        let md = fs::read_to_string(f.dir.path().join(".claude/skills/beta/SKILL.md")).unwrap();
        assert!(md.starts_with("---\nname: beta"));
    }

    #[test]
    fn agent_json_tem_a_identidade_e_nenhum_segredo() {
        let f = fixture();
        let revisor = agent(&f.team, "revisor", &[]);
        let done = run(&f, &revisor, &[builtin("alfa")]);
        let json = fs::read_to_string(done.agent_dir.join("agent.json")).unwrap();
        let card: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(card.handle, "revisor");
        assert_eq!(card.team.mission, "Migrar a autenticação.");
        assert_eq!(card.runtime, "claude");
        assert_eq!(
            card.skills[0].path,
            ".aisense/agents/revisor/skills/alfa/SKILL.md"
        );
        assert!(!json.to_lowercase().contains("token"));
    }

    #[test]
    fn nunca_sobrescreve_skill_nativa_do_usuario_nem_a_apaga() {
        let f = fixture();
        let mine = f.dir.path().join(".claude/skills/alfa");
        fs::create_dir_all(&mine).unwrap();
        fs::write(mine.join("SKILL.md"), "minha versão").unwrap();

        let revisor = agent(&f.team, "revisor", &[]);
        let done = run(&f, &revisor, &[builtin("alfa")]);
        assert_eq!(done.warnings.len(), 1, "{:?}", done.warnings);
        run(&f, &revisor, &[]);
        assert_eq!(
            fs::read_to_string(mine.join("SKILL.md")).unwrap(),
            "minha versão"
        );
    }

    #[test]
    fn pasta_nativa_compartilhada_so_sai_com_o_ultimo_dono() {
        let f = fixture();
        let a = agent(&f.team, "backend", &[]);
        let b = agent(&f.team, "frontend", std::slice::from_ref(&a));
        run(&f, &a, &[builtin("comum")]);
        run(&f, &b, &[builtin("comum")]);
        run(&f, &a, &[]);
        assert!(
            f.dir.path().join(".claude/skills/comum").exists(),
            "frontend ainda usa"
        );
        run(&f, &b, &[]);
        assert!(!f.dir.path().join(".claude/skills/comum").exists());
        // As pastas de cada agente são separadas.
        assert!(f
            .dir
            .path()
            .join(".aisense/agents/backend/agent.json")
            .exists());
        assert!(f
            .dir
            .path()
            .join(".aisense/agents/frontend/agent.json")
            .exists());
    }

    #[test]
    fn copia_arquivos_de_apoio_da_skill_do_usuario() {
        let f = fixture();
        let src = tempfile::tempdir().unwrap();
        let md = "---\nname: rev\ndescription: Revisa.\n---\ncorpo\n";
        fs::write(src.path().join("SKILL.md"), md).unwrap();
        fs::create_dir_all(src.path().join("references")).unwrap();
        fs::write(src.path().join("references/checklist.md"), "- a").unwrap();
        let skill = parse_skill(
            md,
            "t",
            SkillSource::User {
                dir: src.path().display().to_string(),
            },
        )
        .unwrap();
        let revisor = agent(&f.team, "revisor", &[]);
        let done = run(&f, &revisor, &[skill]);
        assert_eq!(
            files(&done.agent_dir.join("skills")),
            ["rev/SKILL.md", "rev/references/checklist.md"]
        );
    }

    #[test]
    fn handle_renomeado_leva_embora_a_pasta_antiga() {
        let f = fixture();
        let mut revisor = agent(&f.team, "revisor", &[]);
        run(&f, &revisor, &[builtin("alfa")]);
        revisor.handle = crate::agent::Handle::parse("critico").unwrap();
        run(&f, &revisor, &[]);
        assert_eq!(
            files(&f.dir.path().join(".aisense/agents")),
            ["critico/agent.json"]
        );
        // O dono antigo da skill nativa era o mesmo agente: ela sai também.
        assert!(!f.dir.path().join(".claude/skills/alfa").exists());
    }

    #[test]
    fn gitignore_e_escrito_uma_vez_e_preserva_as_notas() {
        let f = fixture();
        let revisor = agent(&f.team, "revisor", &[]);
        run(&f, &revisor, &[]);
        let path = f.dir.path().join(".aisense/.gitignore");
        assert!(fs::read_to_string(&path).unwrap().contains("!/notes/"));
        fs::write(&path, "meu").unwrap();
        run(&f, &revisor, &[]);
        assert_eq!(fs::read_to_string(&path).unwrap(), "meu");
    }

    #[test]
    fn skills_dir_fora_do_diretorio_e_recusado() {
        assert!(native_root(Path::new("/w"), "../fora").is_none());
        assert!(native_root(Path::new("/w"), "/etc").is_none());
        assert_eq!(
            native_root(Path::new("/w"), ".claude/skills"),
            Some(PathBuf::from("/w/.claude/skills"))
        );
    }
}
