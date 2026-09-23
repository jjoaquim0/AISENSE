//! O pouco de `git` que as bancadas usam, pelo binário do usuário (`docs/16`).
//!
//! Via linha de comando e não por biblioteca: é o mesmo git que o agente e o humano
//! usam no terminal, com a mesma configuração, hooks e credenciais.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Worktree exige git 2.5.
const MIN_VERSION: (u32, u32) = (2, 5);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GitError {
    #[error("git was not found in PATH")]
    NotInstalled,
    #[error("git {found} is too old for worktrees (2.5 or newer is required)")]
    TooOld { found: String },
    #[error("`git {command}` failed: {stderr}")]
    Failed { command: String, stderr: String },
}

fn git(dir: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        // Sem prompt de credencial ou de editor: um git esperando teclado travaria o start.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                GitError::NotInstalled
            } else {
                GitError::Failed {
                    command: args.join(" "),
                    stderr: e.to_string(),
                }
            }
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned())
    } else {
        Err(GitError::Failed {
            command: args.join(" "),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

/// Confere que há um git com suporte a worktree.
pub fn check_version() -> Result<(), GitError> {
    let text = git(Path::new("."), &["--version"])?;
    // "git version 2.54.0.windows.1"
    let version = text
        .split_whitespace()
        .nth(2)
        .unwrap_or_default()
        .to_owned();
    let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let found = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    if found >= MIN_VERSION {
        Ok(())
    } else {
        Err(GitError::TooOld { found: version })
    }
}

/// Raiz do repositório que contém `dir`, ou `None` se não for um repositório.
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    git(dir, &["rev-parse", "--show-toplevel"])
        .ok()
        .map(PathBuf::from)
}

/// Branch atual. `None` com HEAD destacado.
pub fn current_branch(repo: &Path) -> Result<Option<String>, GitError> {
    let name = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok((name != "HEAD").then_some(name))
}

pub fn branch_exists(repo: &Path, branch: &str) -> bool {
    git(
        repo,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
}

/// Caminhos dos worktrees registrados no repositório (o principal incluído).
pub fn worktrees(repo: &Path) -> Result<Vec<PathBuf>, GitError> {
    let list = git(repo, &["worktree", "list", "--porcelain"])?;
    Ok(list
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect())
}

/// `git worktree add`: cria o branch a partir de `base`, ou reaproveita se já existe.
pub fn add_worktree(repo: &Path, path: &Path, branch: &str, base: &str) -> Result<(), GitError> {
    let path = path.to_string_lossy();
    if branch_exists(repo, branch) {
        git(repo, &["worktree", "add", &path, branch])?;
    } else {
        git(repo, &["worktree", "add", "-b", branch, &path, base])?;
    }
    Ok(())
}

/// Linhas do `git status --porcelain`: mudanças não commitadas, inclusive arquivos novos.
pub fn pending_changes(worktree: &Path) -> Result<Vec<String>, GitError> {
    Ok(git(worktree, &["status", "--porcelain"])?
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Remove pelo git — nunca `rm -rf`, que deixaria o worktree registrado como fantasma.
pub fn remove_worktree(repo: &Path, path: &Path) -> Result<(), GitError> {
    git(repo, &["worktree", "remove", &path.to_string_lossy()])?;
    Ok(())
}

/// Esquece registros de worktrees cujas pastas sumiram.
pub fn prune_worktrees(repo: &Path) -> Result<(), GitError> {
    git(repo, &["worktree", "prune"])?;
    Ok(())
}

pub fn init_submodules(worktree: &Path) -> Result<(), GitError> {
    git(worktree, &["submodule", "update", "--init", "--recursive"])?;
    Ok(())
}

/// Compara caminhos como o sistema compara (maiúsculas e barras no Windows).
pub fn same_path(a: &Path, b: &Path) -> bool {
    let normalize = |p: &Path| {
        let canonical = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let text = canonical.to_string_lossy().replace('\\', "/");
        let text = text
            .trim_start_matches("//?/")
            .trim_end_matches('/')
            .to_owned();
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text
        }
    };
    normalize(a) == normalize(b)
}
