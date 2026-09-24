//! Bancadas: um `git worktree` com branch próprio por agente (`docs/16`).
//!
//! Dois agentes no mesmo checkout se atropelam. Em modo `per-agent`, cada um trabalha
//! em `<benches>/<team_id>/<handle>`, no branch `aisense/<handle>` — fora do
//! repositório, para não poluir o `git status` nem ser commitado por acidente.

pub mod git;
pub mod local;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agent::{Agent, Workbench, WorkspaceMode};
use crate::project::ProjectConfig;
use crate::team::Team;
use git::GitError;

/// Prefixo dos branches das bancadas.
pub const BRANCH_PREFIX: &str = "aisense/";
/// `bench.setup` roda uma vez, na criação; `pnpm install` num projeto grande demora.
pub const SETUP_TIMEOUT: Duration = Duration::from_secs(600);

/// Onde o agente vai trabalhar, e por quê.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Workdir {
    pub path: String,
    /// Presente quando o agente está numa bancada própria.
    pub bench: Option<BenchInfo>,
    /// Limitação a mostrar na UI (sem git, setup que falhou...). Não impede o start.
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct BenchInfo {
    pub path: String,
    pub branch: String,
    /// Branch de onde a bancada nasceu; `None` ao reaproveitar uma existente.
    pub base: Option<String>,
    /// `true` na primeira vez; `false` quando uma bancada existente foi reaproveitada.
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("the bench folder {0} exists but is not a git worktree of this repository")]
    Occupied(String),
    #[error("the repository has no current branch (detached HEAD); choose a base branch")]
    NoBaseBranch,
    #[error(
        "the bench of @{handle} has {count} uncommitted change(s); commit or discard them first"
    )]
    Dirty {
        handle: String,
        count: usize,
        changes: Vec<String>,
    },
    #[error("could not prepare the bench: {0}")]
    Io(String),
}

impl BenchError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Git(_) => "git_failed",
            Self::Occupied(_) => "bench_occupied",
            Self::NoBaseBranch => "no_base_branch",
            Self::Dirty { .. } => "bench_dirty",
            Self::Io(_) => "bench_io",
        }
    }
}

/// O agente decide sobre a equipe: `own`/`shared` ganham de `inherit`.
pub fn effective_mode(team: &Team, agent: &Agent) -> WorkspaceMode {
    match agent.workbench {
        Workbench::Own => WorkspaceMode::PerAgent,
        Workbench::Shared => WorkspaceMode::Shared,
        Workbench::Inherit => team.workspace_mode,
    }
}

pub fn bench_path(benches_root: &Path, team: &Team, agent: &Agent) -> PathBuf {
    benches_root
        .join(team.id.as_str())
        .join(agent.handle.as_str())
}

pub fn bench_branch(agent: &Agent) -> String {
    format!("{BRANCH_PREFIX}{}", agent.handle.as_str())
}

fn shared_workdir(team: &Team, agent: &Agent, warning: Option<String>) -> Workdir {
    Workdir {
        path: agent
            .workdir
            .clone()
            .unwrap_or_else(|| team.workdir.clone()),
        bench: None,
        warning,
    }
}

/// Decide e prepara o diretório de trabalho do agente. Bloqueia: pode criar um worktree,
/// copiar arquivos e rodar o `bench.setup` (até [`SETUP_TIMEOUT`]).
pub fn prepare_workdir(
    team: &Team,
    agent: &Agent,
    benches_root: &Path,
    project: Option<&ProjectConfig>,
) -> Result<Workdir, BenchError> {
    if effective_mode(team, agent) == WorkspaceMode::Shared {
        return Ok(shared_workdir(team, agent, None));
    }
    if let Err(error) = git::check_version() {
        return Ok(shared_workdir(
            team,
            agent,
            Some(format!(
                "Sem bancada própria: {error}. O agente usa o diretório da equipe."
            )),
        ));
    }
    let Some(repo) = git::repo_root(Path::new(&team.workdir)) else {
        return Ok(shared_workdir(
            team,
            agent,
            Some(
                "Sem bancada própria: o diretório da equipe não é um repositório git. \
                 O agente usa o diretório da equipe."
                    .to_owned(),
            ),
        ));
    };

    let path = bench_path(benches_root, team, agent);
    let branch = bench_branch(agent);
    let registered = git::worktrees(&repo)?
        .iter()
        .any(|w| git::same_path(w, &path));
    if registered && path.is_dir() {
        return Ok(Workdir {
            path: path.display().to_string(),
            bench: Some(BenchInfo {
                path: path.display().to_string(),
                base: None,
                branch,
                created: false,
            }),
            warning: None,
        });
    }
    // Registro de uma pasta que sumiu: esquecer antes de recriar.
    git::prune_worktrees(&repo)?;
    if path.exists() && !is_empty_dir(&path) {
        return Err(BenchError::Occupied(path.display().to_string()));
    }
    let base = git::current_branch(&repo)?.ok_or(BenchError::NoBaseBranch)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| BenchError::Io(e.to_string()))?;
    }
    git::add_worktree(&repo, &path, &branch, &base)?;
    tracing::info!(agent = %agent.id, path = %path.display(), %branch, "bancada criada");

    let mut warnings = Vec::new();
    if repo.join(".gitmodules").is_file() {
        if let Err(error) = git::init_submodules(&path) {
            warnings.push(format!("Submódulos não inicializados: {error}"));
        }
    }
    if let Some(project) = project {
        let missing = copy_ignored_files(&repo, &path, &project.bench.copy)?;
        if !missing.is_empty() {
            warnings.push(format!(
                "Não encontrados para copiar: {}",
                missing.join(", ")
            ));
        }
        if let Some(setup) = &project.bench.setup {
            if let Err(error) = run_setup(&path, setup, SETUP_TIMEOUT) {
                warnings.push(format!("O preparo da bancada (`{setup}`) falhou: {error}"));
            }
        }
    }

    Ok(Workdir {
        path: path.display().to_string(),
        bench: Some(BenchInfo {
            path: path.display().to_string(),
            branch,
            base: Some(base),
            created: true,
        }),
        warning: (!warnings.is_empty()).then(|| warnings.join(" · ")),
    })
}

/// Mudanças não commitadas da bancada do agente; vazio quando ela não existe.
pub fn pending_changes(
    benches_root: &Path,
    team: &Team,
    agent: &Agent,
) -> Result<Vec<String>, BenchError> {
    let path = bench_path(benches_root, team, agent);
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    Ok(git::pending_changes(&path)?)
}

/// Remove a bancada do agente, se houver. **Recusa** com mudanças não commitadas:
/// apagar trabalho de um agente é o tipo de coisa que não tem volta.
pub fn remove_bench(benches_root: &Path, team: &Team, agent: &Agent) -> Result<bool, BenchError> {
    let path = bench_path(benches_root, team, agent);
    if !path.is_dir() {
        return Ok(false);
    }
    let changes = git::pending_changes(&path)?;
    if !changes.is_empty() {
        return Err(BenchError::Dirty {
            handle: agent.handle.as_str().to_owned(),
            count: changes.len(),
            changes,
        });
    }
    let repo = git::repo_root(Path::new(&team.workdir))
        .ok_or_else(|| BenchError::Io("the team directory is no longer a repository".into()))?;
    git::remove_worktree(&repo, &path)?;
    tracing::info!(agent = %agent.id, path = %path.display(), "bancada removida");
    Ok(true)
}

fn is_empty_dir(path: &Path) -> bool {
    std::fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}

/// Copia os arquivos ignorados declarados em `bench.copy` (`.env`, configs locais).
/// Devolve os que não existiam no repositório.
fn copy_ignored_files(
    repo: &Path,
    bench: &Path,
    entries: &[String],
) -> Result<Vec<String>, BenchError> {
    let mut missing = Vec::new();
    for entry in entries {
        let from = repo.join(entry);
        let to = bench.join(entry);
        if !from.exists() {
            missing.push(entry.clone());
            continue;
        }
        copy_recursively(&from, &to).map_err(|e| BenchError::Io(format!("{entry}: {e}")))?;
    }
    Ok(missing)
}

fn copy_recursively(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_recursively(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from, to).map(|_| ())
    }
}

/// Roda `bench.setup` no shell do sistema, dentro da bancada.
fn run_setup(dir: &Path, command: &str, timeout: Duration) -> Result<(), String> {
    let mut child = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    }
    .current_dir(dir)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .map_err(|e| e.to_string())?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(status
                    .code()
                    .map_or_else(|| "interrompido".to_owned(), |c| format!("código {c}")))
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("passou de {} min", timeout.as_secs() / 60));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests;
