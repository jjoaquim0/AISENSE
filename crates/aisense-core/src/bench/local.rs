//! `aisense bench` (F05-13, `docs/16`): o agente olha e mexe na própria bancada, pelo git
//! do usuário. `sync` usa **merge, nunca rebase** — a bancada é um branch que o AISENSE
//! também referencia, e reescrever o histórico dela quebraria o worktree.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::git::{git, GitError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchStatus {
    pub path: String,
    pub branch: Option<String>,
    /// A branch do checkout principal da equipe; `None` quando este **é** o principal
    /// (equipe em modo compartilhado).
    pub base: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// `git status --porcelain`, uma linha por arquivo.
    pub pending: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: Option<String>,
    /// O checkout principal (o diretório da equipe).
    pub main: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Published {
    pub branch: String,
    /// Página para abrir o PR, quando o remoto é do GitHub.
    pub compare_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum BenchOpError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("this folder is the team's main checkout, not a bench")]
    NotABench,
    #[error("merging {base} conflicts with your changes in: {files}")]
    Conflict { base: String, files: String },
}

impl BenchOpError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Git(_) => "git_failed",
            Self::NotABench => "not_a_bench",
            Self::Conflict { .. } => "merge_conflict",
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Self::NotABench => Some(
                "A equipe trabalha no diretório compartilhado; não há o que sincronizar.".into(),
            ),
            Self::Conflict { base, .. } => Some(format!(
                "Nada foi mudado. Rode `git merge {base}`, resolva os conflitos e commite."
            )),
            Self::Git(_) => None,
        }
    }
}

fn parse_worktrees(porcelain: &str) -> Vec<WorktreeInfo> {
    let mut list = Vec::new();
    let mut current: Option<WorktreeInfo> = None;
    for line in porcelain.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(done) = current.take() {
                list.push(done);
            }
            current = Some(WorktreeInfo {
                path: path.to_owned(),
                branch: None,
                main: list.is_empty(),
            });
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            if let Some(wt) = current.as_mut() {
                wt.branch = Some(branch.to_owned());
            }
        }
    }
    list.extend(current);
    list
}

/// Todos os worktrees do repositório; o primeiro é o principal.
pub fn list(dir: &Path) -> Result<Vec<WorktreeInfo>, BenchOpError> {
    Ok(parse_worktrees(&git(
        dir,
        &["worktree", "list", "--porcelain"],
    )?))
}

fn same(a: &Path, b: &Path) -> bool {
    super::git::same_path(a, b)
}

fn toplevel(dir: &Path) -> Result<PathBuf, BenchOpError> {
    Ok(PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?))
}

/// Branch base: a do checkout principal, se `dir` não for ele.
fn base(dir: &Path) -> Result<Option<String>, BenchOpError> {
    let top = toplevel(dir)?;
    let all = list(dir)?;
    Ok(match all.first() {
        Some(main) if !same(Path::new(&main.path), &top) => main.branch.clone(),
        _ => None,
    })
}

pub fn status(dir: &Path) -> Result<BenchStatus, BenchOpError> {
    let top = toplevel(dir)?;
    let branch = git(dir, &["branch", "--show-current"])
        .ok()
        .filter(|b| !b.is_empty());
    let base = base(dir)?;
    let (mut ahead, mut behind) = (0, 0);
    if let Some(base) = &base {
        let counts = git(
            dir,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{base}...HEAD"),
            ],
        )?;
        let mut parts = counts
            .split_whitespace()
            .map(|n| n.parse::<u32>().unwrap_or(0));
        behind = parts.next().unwrap_or(0);
        ahead = parts.next().unwrap_or(0);
    }
    let pending = git(dir, &["status", "--porcelain"])?
        .lines()
        .map(str::to_owned)
        .collect();
    Ok(BenchStatus {
        path: top.display().to_string(),
        branch,
        base,
        ahead,
        behind,
        pending,
    })
}

/// Traz a base para dentro da bancada (merge). Conflito: desfaz e explica.
pub fn sync(dir: &Path) -> Result<String, BenchOpError> {
    let base = base(dir)?.ok_or(BenchOpError::NotABench)?;
    match git(dir, &["merge", "--no-edit", &base]) {
        Ok(out) => Ok(out),
        Err(error) => {
            let files = git(dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
            if files.trim().is_empty() {
                return Err(error.into());
            }
            let _ = git(dir, &["merge", "--abort"]);
            Err(BenchOpError::Conflict {
                base,
                files: files.lines().collect::<Vec<_>>().join(", "),
            })
        }
    }
}

/// O que a bancada mudou desde que saiu da base (commitado ou não).
pub fn diff(dir: &Path) -> Result<String, BenchOpError> {
    let base = base(dir)?.ok_or(BenchOpError::NotABench)?;
    let fork = git(dir, &["merge-base", &base, "HEAD"])?;
    Ok(git(dir, &["diff", &fork])?)
}

/// `git push -u origin <branch>` e, no GitHub, a página do PR.
pub fn publish(dir: &Path) -> Result<Published, BenchOpError> {
    let branch = git(dir, &["branch", "--show-current"])?;
    let base = base(dir)?.ok_or(BenchOpError::NotABench)?;
    git(dir, &["push", "-u", "origin", &branch])?;
    let remote = git(dir, &["remote", "get-url", "origin"]).unwrap_or_default();
    Ok(Published {
        compare_url: github_repo(&remote)
            .map(|repo| format!("https://github.com/{repo}/compare/{base}...{branch}?expand=1")),
        branch,
    })
}

/// `owner/repo` de um remoto do GitHub (https ou ssh).
fn github_repo(remote: &str) -> Option<String> {
    let rest = remote
        .strip_prefix("https://github.com/")
        .or_else(|| remote.strip_prefix("git@github.com:"))
        .or_else(|| remote.strip_prefix("ssh://git@github.com/"))?;
    let repo = rest.trim_end_matches('/').trim_end_matches(".git");
    (repo.split('/').count() == 2).then(|| repo.to_owned())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::process::Command;

    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            ok.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&ok.stderr)
        );
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let main = root.path().join("main");
        std::fs::create_dir_all(&main).unwrap();
        run(&main, &["init", "-q", "-b", "main"]);
        run(&main, &["config", "user.name", "AISENSE"]);
        run(&main, &["config", "user.email", "t@aisense.local"]);
        std::fs::write(main.join("a.txt"), "1\n").unwrap();
        run(&main, &["add", "a.txt"]);
        run(&main, &["commit", "-q", "-m", "inicial"]);
        let bench = root.path().join("bench");
        run(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "aisense/backend",
                bench.to_str().unwrap(),
            ],
        );
        (root, main, bench)
    }

    #[test]
    fn status_diff_sync_e_list_numa_bancada_de_verdade() {
        let (_root, main, bench) = setup();
        std::fs::write(bench.join("b.txt"), "novo\n").unwrap();
        run(&bench, &["add", "b.txt"]);
        run(&bench, &["commit", "-q", "-m", "b"]);
        std::fs::write(main.join("c.txt"), "da base\n").unwrap();
        run(&main, &["add", "c.txt"]);
        run(&main, &["commit", "-q", "-m", "c"]);
        std::fs::write(bench.join("rascunho.txt"), "x").unwrap();

        let s = status(&bench).unwrap();
        assert_eq!(s.branch.as_deref(), Some("aisense/backend"));
        assert_eq!(s.base.as_deref(), Some("main"));
        assert_eq!((s.ahead, s.behind), (1, 1));
        assert_eq!(s.pending, ["?? rascunho.txt"]);

        let d = diff(&bench).unwrap();
        assert!(d.contains("b.txt") && !d.contains("c.txt"), "{d}");

        sync(&bench).unwrap();
        assert!(bench.join("c.txt").exists(), "a base entrou por merge");
        let s = status(&bench).unwrap();
        assert_eq!(s.behind, 0);
        // Merge, não rebase: o commit da bancada continua com o mesmo id.
        let merges = Command::new("git")
            .arg("-C")
            .arg(&bench)
            .args(["rev-list", "--merges", "--count", "HEAD"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&merges.stdout).trim(), "1");

        let all = list(&bench).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all[0].main && all[0].branch.as_deref() == Some("main"));
        assert_eq!(all[1].branch.as_deref(), Some("aisense/backend"));

        // No checkout principal não há bancada para sincronizar.
        assert!(matches!(sync(&main), Err(BenchOpError::NotABench)));
        assert_eq!(status(&main).unwrap().base, None);
    }

    #[test]
    fn conflito_no_sync_desfaz_e_explica() {
        let (_root, main, bench) = setup();
        std::fs::write(bench.join("a.txt"), "da bancada\n").unwrap();
        run(&bench, &["commit", "-q", "-am", "bancada"]);
        std::fs::write(main.join("a.txt"), "da base\n").unwrap();
        run(&main, &["commit", "-q", "-am", "base"]);
        let err = sync(&bench).unwrap_err();
        assert_eq!(err.code(), "merge_conflict");
        assert!(err.to_string().contains("a.txt"));
        assert!(err.hint().unwrap().contains("git merge main"));
        assert_eq!(
            std::fs::read_to_string(bench.join("a.txt")).unwrap(),
            "da bancada\n"
        );
        assert!(status(&bench).unwrap().pending.is_empty(), "merge desfeito");
    }

    #[test]
    fn url_do_pr_so_para_o_github() {
        assert_eq!(
            github_repo("https://github.com/a/b.git").as_deref(),
            Some("a/b")
        );
        assert_eq!(
            github_repo("git@github.com:a/b.git").as_deref(),
            Some("a/b")
        );
        assert_eq!(github_repo("https://gitlab.com/a/b.git"), None);
    }
}
