//! Bancadas com repositórios git de verdade, em pastas temporárias.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

use super::*;
use crate::agent::AgentDraft;
use crate::project::parse_project_config;
use crate::team::TeamDraft;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("aisense-{tag}-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `git` com identidade fixa: a máquina de CI não tem `user.name` configurado.
fn run_git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=AISENSE",
            "-c",
            "user.email=teste@aisense.local",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Repositório com um commit em `main` e um `.env` ignorado.
fn repo() -> TempDir {
    let dir = TempDir::new("repo");
    run_git(&dir.0, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.0.join("app.txt"), "versão original\n").unwrap();
    std::fs::write(dir.0.join(".gitignore"), ".env\n").unwrap();
    std::fs::write(dir.0.join(".env"), "SEGREDO=1\n").unwrap();
    run_git(&dir.0, &["add", "app.txt", ".gitignore"]);
    run_git(&dir.0, &["commit", "-q", "-m", "inicial"]);
    dir
}

fn team(workdir: &Path, mode: WorkspaceMode) -> Team {
    Team::create(
        &TeamDraft {
            name: "Squad".into(),
            workdir: workdir.display().to_string(),
            workspace_mode: mode,
            ..TeamDraft::default()
        },
        1,
    )
    .unwrap()
}

fn agent(team: &Team, handle: &str) -> Agent {
    Agent::create(
        team.id.clone(),
        &AgentDraft {
            handle: handle.into(),
            name: handle.into(),
            adapter_id: "shell".into(),
            ..AgentDraft::default()
        },
        &[],
        1,
    )
    .unwrap()
}

#[test]
fn two_agents_edit_the_same_file_without_clobbering() {
    let repo = repo();
    let benches = TempDir::new("benches");
    let team = team(&repo.0, WorkspaceMode::PerAgent);
    let (back, front) = (agent(&team, "backend"), agent(&team, "frontend"));

    let a = prepare_workdir(&team, &back, &benches.0, None).unwrap();
    let b = prepare_workdir(&team, &front, &benches.0, None).unwrap();
    assert_ne!(a.path, b.path);
    assert_eq!(a.bench.as_ref().unwrap().branch, "aisense/backend");
    assert_eq!(a.bench.as_ref().unwrap().base.as_deref(), Some("main"));

    // Cada um escreve o mesmo arquivo do seu jeito e commita na própria bancada.
    for (workdir, text) in [
        (&a, "backend esteve aqui\n"),
        (&b, "frontend esteve aqui\n"),
    ] {
        let dir = Path::new(&workdir.path);
        std::fs::write(dir.join("app.txt"), text).unwrap();
        run_git(dir, &["commit", "-q", "-am", text.trim()]);
    }

    assert_eq!(
        std::fs::read_to_string(Path::new(&a.path).join("app.txt")).unwrap(),
        "backend esteve aqui\n"
    );
    assert_eq!(
        std::fs::read_to_string(Path::new(&b.path).join("app.txt")).unwrap(),
        "frontend esteve aqui\n"
    );
    // O checkout da equipe não foi tocado.
    assert_eq!(
        std::fs::read_to_string(repo.0.join("app.txt")).unwrap(),
        "versão original\n"
    );
    assert_eq!(
        run_git(&repo.0, &["log", "-1", "--format=%s", "aisense/frontend"]),
        "frontend esteve aqui"
    );
}

#[test]
fn an_existing_bench_is_reused() {
    let repo = repo();
    let benches = TempDir::new("benches");
    let team = team(&repo.0, WorkspaceMode::PerAgent);
    let dev = agent(&team, "dev");

    let first = prepare_workdir(&team, &dev, &benches.0, None).unwrap();
    std::fs::write(Path::new(&first.path).join("rascunho.txt"), "wip").unwrap();
    let second = prepare_workdir(&team, &dev, &benches.0, None).unwrap();
    assert_eq!(first.path, second.path);
    assert!(!second.bench.unwrap().created);
    assert!(
        Path::new(&second.path).join("rascunho.txt").exists(),
        "trabalho em andamento não se joga fora"
    );
}

#[test]
fn deleting_a_bench_with_pending_work_is_refused() {
    let repo = repo();
    let benches = TempDir::new("benches");
    let team = team(&repo.0, WorkspaceMode::PerAgent);
    let dev = agent(&team, "dev");
    let workdir = prepare_workdir(&team, &dev, &benches.0, None).unwrap();

    std::fs::write(Path::new(&workdir.path).join("app.txt"), "mudança\n").unwrap();
    let error = remove_bench(&benches.0, &team, &dev).unwrap_err();
    assert_eq!(error.code(), "bench_dirty");
    assert!(error.to_string().contains("@dev"), "{error}");
    assert!(error.to_string().contains("uncommitted"), "{error}");
    assert!(Path::new(&workdir.path).exists(), "nada foi apagado");

    // Commitado, a remoção passa — e pelo git, sem worktree fantasma.
    run_git(Path::new(&workdir.path), &["commit", "-q", "-am", "salvo"]);
    assert!(remove_bench(&benches.0, &team, &dev).unwrap());
    assert!(!Path::new(&workdir.path).exists());
    let registered = run_git(&repo.0, &["worktree", "list"]);
    assert!(!registered.contains("dev"), "{registered}");
    // O branch com o trabalho continua lá.
    assert_eq!(
        run_git(&repo.0, &["log", "-1", "--format=%s", "aisense/dev"]),
        "salvo"
    );
}

#[test]
fn not_a_repository_falls_back_to_shared_with_a_warning() {
    let plain = TempDir::new("plain");
    let benches = TempDir::new("benches");
    let team = team(&plain.0, WorkspaceMode::PerAgent);
    let workdir = prepare_workdir(&team, &agent(&team, "dev"), &benches.0, None).unwrap();
    assert_eq!(workdir.path, team.workdir);
    assert!(workdir.bench.is_none());
    assert!(workdir
        .warning
        .unwrap()
        .contains("não é um repositório git"));
}

#[test]
fn agent_exception_wins_over_the_team_mode() {
    let repo = repo();
    let benches = TempDir::new("benches");
    let per_agent = team(&repo.0, WorkspaceMode::PerAgent);
    let mut reviewer = agent(&per_agent, "revisor");
    reviewer.workbench = Workbench::Shared;
    let workdir = prepare_workdir(&per_agent, &reviewer, &benches.0, None).unwrap();
    assert!(
        workdir.bench.is_none(),
        "revisor que só lê fica no checkout compartilhado"
    );

    let shared = team(&repo.0, WorkspaceMode::Shared);
    let mut owner = agent(&shared, "dono");
    owner.workbench = Workbench::Own;
    assert!(prepare_workdir(&shared, &owner, &benches.0, None)
        .unwrap()
        .bench
        .is_some());
}

#[test]
fn a_new_bench_gets_ignored_files_and_runs_setup() {
    let repo = repo();
    let benches = TempDir::new("benches");
    let team = team(&repo.0, WorkspaceMode::PerAgent);
    let setup = if cfg!(windows) {
        "echo pronto> preparado.txt"
    } else {
        "echo pronto > preparado.txt"
    };
    let project = parse_project_config(
        &format!("[bench]\ncopy = [\".env\", \"nao-existe.local\"]\nsetup = {setup:?}\n"),
        "aisense.toml",
    )
    .unwrap();

    let workdir = prepare_workdir(&team, &agent(&team, "dev"), &benches.0, Some(&project)).unwrap();
    let dir = Path::new(&workdir.path);
    assert_eq!(
        std::fs::read_to_string(dir.join(".env")).unwrap(),
        "SEGREDO=1\n"
    );
    assert!(
        dir.join("preparado.txt").exists(),
        "o setup rodou dentro da bancada"
    );
    let warning = workdir.warning.unwrap();
    assert!(warning.contains("nao-existe.local"), "{warning}");
    // O `.env` copiado continua ignorado: a bancada segue limpa para o git.
    assert!(git::pending_changes(dir)
        .unwrap()
        .iter()
        .all(|l| !l.contains(".env")));
}

#[test]
fn a_foreign_folder_in_the_bench_path_is_not_overwritten() {
    let repo = repo();
    let benches = TempDir::new("benches");
    let team = team(&repo.0, WorkspaceMode::PerAgent);
    let dev = agent(&team, "dev");
    let path = bench_path(&benches.0, &team, &dev);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("algo.txt"), "não é meu").unwrap();

    let error = prepare_workdir(&team, &dev, &benches.0, None).unwrap_err();
    assert_eq!(error.code(), "bench_occupied");
    assert!(path.join("algo.txt").exists());
}

#[test]
fn an_existing_branch_is_reused_instead_of_failing() {
    let repo = repo();
    let benches = TempDir::new("benches");
    let team = team(&repo.0, WorkspaceMode::PerAgent);
    run_git(&repo.0, &["branch", "aisense/dev"]);
    let workdir = prepare_workdir(&team, &agent(&team, "dev"), &benches.0, None).unwrap();
    assert_eq!(
        run_git(
            Path::new(&workdir.path),
            &["rev-parse", "--abbrev-ref", "HEAD"]
        ),
        "aisense/dev"
    );
}
