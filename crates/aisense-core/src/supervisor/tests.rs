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
    fn state_changed(&self, _agent_id: &AgentId, state: AgentState) {
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
    let runtimes = Arc::new(RuntimeRegistry::new(AdapterCatalog::load_from(
        BUILTIN_ADAPTERS,
        None,
    )));
    let supervisor = AgentSupervisor::new(
        Arc::clone(&store),
        runtimes,
        Arc::clone(&pty),
        Arc::new(Silent),
        Arc::clone(&recorder) as Arc<dyn SupervisorObserver>,
        SupervisorConfig {
            logs_dir: logs.clone(),
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
    assert_eq!(h.supervisor.state(&id), AgentState::Idle);
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
    assert_eq!(
        states,
        vec![AgentState::Starting, AgentState::Idle, AgentState::Stopped]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn killing_the_process_externally_restarts_on_crash() {
    let h = harness();
    let id = h.agent(long_running(), RestartPolicy::OnCrash).await;
    h.supervisor.start(&id).await.unwrap();

    // "Externamente": direto no PTY, sem passar pelo supervisor.
    h.pty.kill(id.as_str()).unwrap();
    h.wait_sessions(&id, 2).await;
    h.wait_for("de pé de novo", |h| {
        h.supervisor.state(&id) == AgentState::Idle
    })
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
    assert_eq!(h.supervisor.state(&id), AgentState::Idle);
    assert_eq!(h.sessions(&id).await.len(), 2);
}
