//! Supervisor com processos reais: PTY de verdade, `InMemoryStore` e o adaptador
//! `custom`, que roda o comando que o agente pedir.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use super::*;
use crate::adapter::{AdapterCatalog, BUILTIN_ADAPTERS};
use crate::agent::{Agent, AgentDraft};
use crate::repo::InMemoryStore;
use crate::team::{Team, TeamDraft};

/// Folga para o ConPTY do Windows 10, lento para subir processos.
const PATIENCE: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Recorder {
    states: Mutex<Vec<AgentState>>,
}

impl SupervisorObserver for Recorder {
    fn state_changed(&self, _agent_id: &AgentId, state: AgentState, _confidence: StateConfidence) {
        self.states.lock().unwrap().push(state);
    }
}

struct Silent;

impl OutputSink for Silent {
    fn data(&self, _agent_id: &str, _chunk: Vec<u8>) {}
    fn exit(&self, _agent_id: &str, _code: i32) {}
}

fn long_running() -> Vec<String> {
    let args: &[&str] = if cfg!(windows) {
        &["ping", "-n", "120", "127.0.0.1"]
    } else {
        &["sleep", "120"]
    };
    args.iter().map(|s| (*s).to_owned()).collect()
}

fn exits_with(code: i32) -> Vec<String> {
    if cfg!(windows) {
        vec!["cmd".into(), "/C".into(), format!("exit {code}")]
    } else {
        vec!["sh".into(), "-c".into(), format!("exit {code}")]
    }
}

/// Adaptador de teste: roda o comando do agente e reconhece o prompt `pronto>`.
const PROMPTY: &str = r#"
id      = "prompty"
name    = "Prompt de teste"
command = "$AGENT_COMMAND"

[state]
idle_regex     = '(?m)^pronto>\s*$'
busy_regex     = 'trabalhando'
awaiting_regex = '\(s/n\)'
quiet_ms       = 150

[inject]
mode = "none"
"#;

struct Harness {
    supervisor: AgentSupervisor<InMemoryStore>,
    store: Arc<InMemoryStore>,
    pty: Arc<PtyManager>,
    recorder: Arc<Recorder>,
    logs: PathBuf,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.supervisor.shutdown();
        self.pty.shutdown();
        let _ = std::fs::remove_dir_all(&self.logs);
    }
}

fn harness() -> Harness {
    let store = Arc::new(InMemoryStore::new());
    let pty = Arc::new(PtyManager::new());
    let recorder = Arc::new(Recorder::default());
    let logs = std::env::temp_dir().join(format!("aisense-sup-{}", ulid::Ulid::new()));
    let mut builtins = BUILTIN_ADAPTERS.to_vec();
    builtins.push(("prompty.toml", PROMPTY));
    let runtimes = Arc::new(RuntimeRegistry::new(AdapterCatalog::load_from(
        &builtins, None,
    )));
    let supervisor = AgentSupervisor::new(
        Arc::clone(&store),
        runtimes,
        Arc::clone(&pty),
        Arc::new(Silent),
        Arc::clone(&recorder) as Arc<dyn SupervisorObserver>,
        SupervisorConfig {
            logs_dir: logs.clone(),
            benches_dir: logs.join("benches"),
            launch: LaunchContext {
                socket: "test.sock".into(),
                sidecar_dir: None,
                inherited_path: std::env::var_os("PATH"),
            },
            size: TerminalSize::default(),
        },
    );
    Harness {
        supervisor,
        store,
        pty,
        recorder,
        logs,
    }
}

impl Harness {
    async fn agent(&self, args: Vec<String>, policy: RestartPolicy) -> AgentId {
        self.agent_with("custom", args, policy).await
    }

    async fn agent_with(&self, adapter: &str, args: Vec<String>, policy: RestartPolicy) -> AgentId {
        let team = Team::create(
            &TeamDraft {
                name: "Squad".into(),
                workdir: std::env::temp_dir().display().to_string(),
                ..TeamDraft::default()
            },
            1,
        )
        .unwrap();
        self.store.create_team(&team).await.unwrap();
        let agent = Agent::create(
            team.id.clone(),
            &AgentDraft {
                handle: "backend".into(),
                name: "Backend".into(),
                adapter_id: adapter.into(),
                args,
                restart_policy: policy,
                ..AgentDraft::default()
            },
            &[],
            1,
        )
        .unwrap();
        self.store.create_agent(&agent).await.unwrap();
        agent.id
    }

    async fn sessions(&self, id: &AgentId) -> Vec<SessionRecord> {
        self.store.list_sessions(id, 50).await.unwrap()
    }

    async fn wait_for(&self, what: &str, mut condition: impl FnMut(&Self) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        while !condition(self) {
            assert!(Instant::now() < deadline, "esperando: {what}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn wait_sessions(&self, id: &AgentId, count: usize) {
        let deadline = Instant::now() + PATIENCE;
        while self.sessions(id).await.len() < count {
            assert!(Instant::now() < deadline, "esperando {count} sessões");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn start_records_a_session_and_stop_is_final() {
    let h = harness();
    let id = h.agent(long_running(), RestartPolicy::Always).await;

    h.supervisor.start(&id).await.unwrap();
    // `sleep` não imprime nada: sem prompt, o detector ainda não decidiu.
    assert_eq!(h.supervisor.state(&id), AgentState::Starting);
    let sessions = h.sessions(&id).await;
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0]
        .log_path
        .ends_with(&format!("{}.log", id.as_str())));

    h.supervisor.stop(&id).unwrap();
    h.wait_for("parado", |h| h.supervisor.state(&id) == AgentState::Stopped)
        .await;
    // Nem com `always` um stop pedido pelo usuário reinicia.
    tokio::time::sleep(FIRST_DELAY * 2).await;
    assert_eq!(h.supervisor.state(&id), AgentState::Stopped);
    let sessions = h.sessions(&id).await;
    assert_eq!(sessions.len(), 1, "não deveria ter reiniciado");
    assert!(sessions[0].ended_at.is_some());

    let states = h.recorder.states.lock().unwrap().clone();
    assert_eq!(states, vec![AgentState::Starting, AgentState::Stopped]);
}

#[tokio::test(flavor = "multi_thread")]
async fn killing_the_process_externally_restarts_on_crash() {
    let h = harness();
    let id = h.agent(long_running(), RestartPolicy::OnCrash).await;
    h.supervisor.start(&id).await.unwrap();

    // "Externamente": direto no PTY, sem passar pelo supervisor.
    h.pty.kill(id.as_str()).unwrap();
    h.wait_sessions(&id, 2).await;
    h.wait_for("de pé de novo", |h| h.supervisor.state(&id).is_running())
        .await;

    let states = h.recorder.states.lock().unwrap().clone();
    assert!(
        states.contains(&AgentState::Failed),
        "a queda aparece antes do reinício: {states:?}"
    );
    let first = &h.sessions(&id).await[1];
    assert!(first.exit_code.is_some_and(|c| c != 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn never_policy_does_not_restart() {
    let h = harness();
    let id = h.agent(exits_with(3), RestartPolicy::Never).await;
    h.supervisor.start(&id).await.unwrap();

    h.wait_for("falhou", |h| h.supervisor.state(&id) == AgentState::Failed)
        .await;
    tokio::time::sleep(FIRST_DELAY * 2).await;
    let sessions = h.sessions(&id).await;
    assert_eq!(sessions.len(), 1, "`never` não reinicia");
    assert_eq!(sessions[0].exit_code, Some(3));
}

#[tokio::test(flavor = "multi_thread")]
async fn on_crash_respects_a_clean_exit() {
    let h = harness();
    let id = h.agent(exits_with(0), RestartPolicy::OnCrash).await;
    h.supervisor.start(&id).await.unwrap();

    h.wait_for("parado", |h| h.supervisor.state(&id) == AgentState::Stopped)
        .await;
    tokio::time::sleep(FIRST_DELAY * 2).await;
    assert_eq!(h.sessions(&id).await.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_cancels_a_pending_restart() {
    let h = harness();
    let id = h.agent(exits_with(1), RestartPolicy::Always).await;
    h.supervisor.start(&id).await.unwrap();
    h.wait_for("falhou", |h| h.supervisor.state(&id) == AgentState::Failed)
        .await;

    // O reinício está agendado para daqui a FIRST_DELAY; parar agora o cancela.
    h.supervisor.stop(&id).unwrap();
    tokio::time::sleep(FIRST_DELAY * 3).await;
    assert_eq!(h.supervisor.state(&id), AgentState::Stopped);
    assert_eq!(h.sessions(&id).await.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn starting_twice_is_refused() {
    let h = harness();
    let id = h.agent(long_running(), RestartPolicy::Never).await;
    h.supervisor.start(&id).await.unwrap();
    let error = h.supervisor.start(&id).await.unwrap_err();
    assert_eq!(error.code(), "already_running");
    assert!(error.hint().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_runtime_fails_with_the_install_hint() {
    let h = harness();
    let id = h
        .agent(vec!["aisense-no-such-cli-42".into()], RestartPolicy::Always)
        .await;
    let error = h.supervisor.start(&id).await.unwrap_err();
    assert_eq!(error.code(), "runtime_not_installed");
    assert_eq!(h.supervisor.state(&id), AgentState::Failed);
    assert!(h.sessions(&id).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_adapter_is_reported() {
    let h = harness();
    let id = h
        .agent_with("sumiu", long_running(), RestartPolicy::Never)
        .await;
    let error = h.supervisor.start(&id).await.unwrap_err();
    assert_eq!(error.code(), "unknown_adapter");
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_brings_up_a_new_session() {
    let h = harness();
    let id = h.agent(long_running(), RestartPolicy::Never).await;
    h.supervisor.start(&id).await.unwrap();
    h.supervisor.restart(&id).await.unwrap();
    assert!(h.supervisor.state(&id).is_running());
    assert_eq!(h.sessions(&id).await.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_session_knows_where_it_starts_in_the_log() {
    let h = harness();
    let echo = if cfg!(windows) {
        vec!["cmd".into(), "/C".into(), "echo sessao".into()]
    } else {
        vec!["sh".into(), "-c".into(), "echo sessao".into()]
    };
    let id = h.agent(echo, RestartPolicy::Never).await;
    for round in 1..=2 {
        h.supervisor.start(&id).await.unwrap();
        h.wait_sessions(&id, round).await;
        h.wait_for("parado", |h| !h.supervisor.state(&id).is_running())
            .await;
    }
    let sessions = h.sessions(&id).await;
    assert_eq!(sessions[1].log_offset, Some(0), "a primeira começa no zero");
    let second = sessions[0].log_offset.unwrap();
    assert!(second > 0, "a segunda começa depois da primeira");

    for session in &sessions {
        let t = crate::transcript::read_transcript(&sessions, &session.id).unwrap();
        assert_eq!(
            t.text.matches("sessao").count(),
            1,
            "cada sessão só com a própria saída: {:?}",
            t.text
        );
    }
}

/// Repositório git com um commit, para os testes de bancada.
fn git_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aisense-sup-repo-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args([
                "-c",
                "user.name=AISENSE",
                "-c",
                "user.email=t@aisense.local",
            ])
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("app.txt"), "v1\n").unwrap();
    git(&["add", "app.txt"]);
    git(&["commit", "-q", "-m", "inicial"]);
    dir
}

#[tokio::test(flavor = "multi_thread")]
async fn a_per_agent_team_starts_each_agent_in_its_own_bench() {
    let h = harness();
    let repo = git_repo();
    let team = Team::create(
        &TeamDraft {
            name: "Paralela".into(),
            workdir: repo.display().to_string(),
            workspace_mode: crate::agent::WorkspaceMode::PerAgent,
            ..TeamDraft::default()
        },
        1,
    )
    .unwrap();
    h.store.create_team(&team).await.unwrap();
    let agent = Agent::create(
        team.id.clone(),
        &AgentDraft {
            handle: "dev".into(),
            name: "Dev".into(),
            adapter_id: "custom".into(),
            args: long_running(),
            restart_policy: RestartPolicy::Never,
            ..AgentDraft::default()
        },
        &[],
        1,
    )
    .unwrap();
    h.store.create_agent(&agent).await.unwrap();

    let outcome = h.supervisor.start(&agent.id).await.unwrap();
    let bench = outcome.workdir.bench.expect("deveria ter bancada própria");
    assert_eq!(bench.branch, "aisense/dev");
    assert!(std::path::Path::new(&bench.path).join("app.txt").exists());

    // Trabalho pendente na bancada: excluir o agente é recusado com motivo claro.
    std::fs::write(std::path::Path::new(&bench.path).join("app.txt"), "wip\n").unwrap();
    let error = h.supervisor.retire(&agent.id).await.unwrap_err();
    assert_eq!(error.code(), "bench_dirty");
    assert!(error.hint().is_some());
    assert!(std::path::Path::new(&bench.path).exists());

    let _ = std::fs::remove_dir_all(&repo);
}

/// F03-01 de ponta a ponta: processo real, PTY real, detector decidindo.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn the_detector_drives_the_state_of_a_real_process() {
    let h = harness();
    let script = "printf 'pronto> '; read a; echo trabalhando; sleep 1; \
                  printf 'Continuar? (s/n) '; read b; printf 'pronto> '; sleep 60";
    let id = h
        .agent_with(
            "prompty",
            vec!["sh".into(), "-c".into(), script.into()],
            RestartPolicy::Never,
        )
        .await;
    h.supervisor.start(&id).await.unwrap();

    h.wait_for("ocioso no prompt", |h| {
        h.supervisor.state(&id) == AgentState::Idle
    })
    .await;
    h.pty.write(id.as_str(), b"vai\r").unwrap();
    h.wait_for("pergunta ao humano", |h| {
        h.supervisor.state(&id) == AgentState::AwaitingInput
    })
    .await;
    h.pty.write(id.as_str(), b"s\r").unwrap();
    h.wait_for("ocioso de novo", |h| {
        h.supervisor.state(&id) == AgentState::Idle
    })
    .await;

    let states = h.recorder.states.lock().unwrap().clone();
    assert_eq!(
        states,
        [
            AgentState::Starting,
            AgentState::Idle,
            AgentState::Busy,
            AgentState::AwaitingInput,
            AgentState::Busy,
            AgentState::Idle,
        ]
    );
}

// ───────────────────── controles da equipe (F03-06) ─────────────────────

impl Harness {
    /// Uma equipe com `autostart` agentes que sobem sozinhos e `manual` que não.
    async fn squad(&self, autostart: usize, manual: usize) -> (TeamId, Vec<AgentId>) {
        let team = Team::create(
            &TeamDraft {
                name: "Squad".into(),
                workdir: std::env::temp_dir().display().to_string(),
                ..TeamDraft::default()
            },
            1,
        )
        .unwrap();
        self.store.create_team(&team).await.unwrap();
        let mut ids = Vec::new();
        for i in 0..autostart + manual {
            let existing = self.store.list_agents(&team.id).await.unwrap();
            let agent = Agent::create(
                team.id.clone(),
                &AgentDraft {
                    handle: format!("agente-{i}"),
                    name: format!("Agente {i}"),
                    adapter_id: "custom".into(),
                    args: long_running(),
                    autostart: i < autostart,
                    restart_policy: RestartPolicy::Never,
                    ..AgentDraft::default()
                },
                &existing,
                1,
            )
            .unwrap();
            self.store.create_agent(&agent).await.unwrap();
            ids.push(agent.id);
        }
        (team.id, ids)
    }
}

/// Grava o progresso com o instante de cada evento.
#[derive(Default)]
struct ProgressLog(Mutex<Vec<(Instant, TeamProgress)>>);

impl ProgressLog {
    fn push(&self, p: TeamProgress) {
        self.0.lock().unwrap().push((Instant::now(), p));
    }
    fn events(&self) -> Vec<(Instant, TeamProgress)> {
        self.0.lock().unwrap().clone()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn starting_a_team_of_six_is_staggered_and_never_blocks_the_runtime() {
    let h = harness();
    let (team, ids) = h.squad(6, 1).await;
    let log = ProgressLog::default();

    // Um "front" fictício no mesmo runtime de uma thread só: se o start da equipe
    // travasse, este contador pararia.
    let ticks = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter = Arc::clone(&ticks);
    let ticker = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    });

    let stagger = Duration::from_millis(100);
    let started = Instant::now();
    let report = h
        .supervisor
        .start_team(&team, stagger, &|p| log.push(p))
        .await
        .unwrap();
    let elapsed = started.elapsed();
    ticker.abort();

    assert!(report.failures.is_empty(), "{:?}", report.failures);
    for id in &ids[..6] {
        assert!(h.supervisor.state(id).is_running(), "autostart agents run");
    }
    assert!(
        !h.supervisor.state(&ids[6]).is_running(),
        "manual agent stays put"
    );

    // Ordem da equipe, um por vez, com o intervalo entre eles.
    let events = log.events();
    let per_agent: Vec<_> = events
        .iter()
        .filter(|(_, p)| p.agent_id.is_some())
        .collect();
    let order: Vec<AgentId> = per_agent
        .iter()
        .filter_map(|(_, p)| p.agent_id.clone())
        .collect();
    assert_eq!(order, ids[..6]);
    for pair in per_agent.windows(2) {
        assert!(pair[1].0 - pair[0].0 >= stagger, "agents must be staggered");
    }
    let last = &events.last().unwrap().1;
    assert!(last.finished && last.done == 6 && last.total == 6);

    let ticked = u128::from(ticks.load(std::sync::atomic::Ordering::Relaxed));
    let expected = elapsed.as_millis() / 10;
    assert!(
        ticked * 2 >= expected,
        "the runtime kept running while the team started ({ticked} ticks in {elapsed:?})"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_all_then_restart_all() {
    let h = harness();
    let (team, ids) = h.squad(3, 1).await;
    h.supervisor
        .start_team(&team, Duration::ZERO, &|_| {})
        .await
        .unwrap();
    // O manual sobe à mão: "parar tudo" também o para.
    h.supervisor.start(&ids[3]).await.unwrap();

    h.supervisor.stop_team(&team, &|_| {}).await.unwrap();
    for id in &ids {
        assert!(!h.supervisor.state(id).is_running());
    }

    // Reiniciar sem ninguém rodando é iniciar a equipe: só os `autostart`.
    h.supervisor
        .restart_team(&team, Duration::ZERO, &|_| {})
        .await
        .unwrap();
    assert!(h.supervisor.state(&ids[0]).is_running());
    assert!(!h.supervisor.state(&ids[3]).is_running());

    // Com gente rodando, reinicia exatamente quem estava de pé: sessão nova para cada.
    let before = h.sessions(&ids[0]).await.len();
    let log = ProgressLog::default();
    h.supervisor
        .restart_team(&team, Duration::ZERO, &|p| log.push(p))
        .await
        .unwrap();
    assert_eq!(h.sessions(&ids[0]).await.len(), before + 1);
    assert!(h.supervisor.state(&ids[0]).is_running());
    assert!(!h.supervisor.state(&ids[3]).is_running());
    assert!(log.events().iter().all(|(_, p)| p.op == TeamOp::Restart));
}
