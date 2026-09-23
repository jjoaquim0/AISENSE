//! Supervisor de agentes (`docs/02`, fluxo 1 e "Máquina de estados do agente").
//!
//! Sobe o processo de cada agente num PTY com o comando e o ambiente certos, registra
//! a sessão, e quando o processo termina decide — pela política do agente — se ele
//! volta, com espera crescente entre quedas seguidas.

mod backoff;
mod launch;
mod token;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use aisense_pty::{OutputSink, PtyError, PtyManager, PtySpawn, TerminalSize};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub use backoff::{Backoff, FIRST_DELAY, MAX_DELAY, STABLE_AFTER};
pub use launch::{build_launch, LaunchContext, LaunchError, LaunchIdentity, LaunchPlan};
pub use token::{generate_token, TokenError};

use crate::adapter::RuntimeRegistry;
use crate::agent::{AgentState, RestartPolicy};
use crate::ids::{AgentId, SessionId, TeamId};
use crate::repo::{AgentRepository, RepoError, SessionRecord, SessionRepository, TeamRepository};
use crate::time::now_ms;

/// Quanto o término de uma sessão espera o registro do início dela. Só importa
/// quando o processo morre antes de o supervisor terminar de gravar o início.
const SESSION_RECORD_WAIT: Duration = Duration::from_secs(5);

/// Payload do evento `agent:state` que o app emite a cada mudança.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentStateChanged {
    pub agent_id: AgentId,
    pub state: AgentState,
}

/// Quem precisa saber das mudanças de estado (o app emite `agent:state`).
pub trait SupervisorObserver: Send + Sync + 'static {
    fn state_changed(&self, agent_id: &AgentId, state: AgentState);
}

/// As portas de que o supervisor precisa, juntas.
pub trait SupervisorStore: TeamRepository + AgentRepository + SessionRepository + 'static {}
impl<T: TeamRepository + AgentRepository + SessionRepository + 'static> SupervisorStore for T {}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// `logs/<agent_id>.log` (`docs/02`, "Persistência e layout em disco").
    pub logs_dir: PathBuf,
    pub launch: LaunchContext,
    /// Tamanho inicial do terminal; a UI redimensiona quando o painel abre.
    pub size: TerminalSize,
}

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("agent {0} not found")]
    AgentNotFound(AgentId),
    #[error("team {0} not found")]
    TeamNotFound(TeamId),
    #[error("runtime {0:?} is not configured")]
    UnknownAdapter(String),
    #[error("agent {0} is already running")]
    AlreadyRunning(AgentId),
    #[error(transparent)]
    Launch(#[from] LaunchError),
    #[error(transparent)]
    Pty(#[from] PtyError),
    #[error(transparent)]
    Repo(#[from] RepoError),
    #[error(transparent)]
    Token(#[from] TokenError),
}

impl SupervisorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AgentNotFound(_) => "agent_not_found",
            Self::TeamNotFound(_) => "team_not_found",
            Self::UnknownAdapter(_) => "unknown_adapter",
            Self::AlreadyRunning(_) => "already_running",
            Self::Launch(LaunchError::NotInstalled { .. }) => "runtime_not_installed",
            Self::Launch(LaunchError::MissingCustomCommand) => "missing_custom_command",
            Self::Pty(PtyError::MissingWorkdir(_)) => "missing_workdir",
            Self::Pty(_) => "pty_failed",
            Self::Repo(e) => e.code(),
            Self::Token(_) => "token_failed",
        }
    }

    /// Próximo passo para o usuário, quando há um.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Launch(LaunchError::NotInstalled {
                install_hint: Some(hint),
                ..
            }) => Some(format!("Instale com: {hint}")),
            Self::Launch(LaunchError::NotInstalled { .. }) => {
                Some("Instale o runtime ou escolha outro para o agente.".to_owned())
            }
            Self::Launch(LaunchError::MissingCustomCommand) => {
                Some("Informe o comando no primeiro argumento do agente.".to_owned())
            }
            Self::UnknownAdapter(_) => {
                Some("Escolha outro runtime ou restaure o arquivo do adaptador.".to_owned())
            }
            Self::AlreadyRunning(_) => Some("Pare o agente antes de iniciá-lo de novo.".to_owned()),
            Self::Pty(e) => e.hint().map(str::to_owned),
            _ => None,
        }
    }
}

pub type SupervisorResult<T> = Result<T, SupervisorError>;

/// Estado de um agente sob supervisão.
struct Supervised {
    state: AgentState,
    /// Sessão atual. Um término de outra sessão (antiga) é ignorado.
    session: Option<SessionId>,
    started: Instant,
    backoff: Backoff,
    restart_policy: RestartPolicy,
    /// Parar o agente cancela o reinício agendado (`docs/02`, "Concorrência").
    cancel: CancellationToken,
    stop_requested: bool,
}

impl Supervised {
    fn new() -> Self {
        Self {
            state: AgentState::Stopped,
            session: None,
            started: Instant::now(),
            backoff: Backoff::default(),
            restart_policy: RestartPolicy::default(),
            cancel: CancellationToken::new(),
            stop_requested: false,
        }
    }
}

struct Shared<S> {
    store: Arc<S>,
    runtimes: Arc<RuntimeRegistry>,
    pty: Arc<PtyManager>,
    output: Arc<dyn OutputSink>,
    observer: Arc<dyn SupervisorObserver>,
    config: SupervisorConfig,
    agents: Mutex<HashMap<AgentId, Supervised>>,
}

pub struct AgentSupervisor<S> {
    shared: Arc<Shared<S>>,
}

impl<S> Clone for AgentSupervisor<S> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<S: SupervisorStore> AgentSupervisor<S> {
    pub fn new(
        store: Arc<S>,
        runtimes: Arc<RuntimeRegistry>,
        pty: Arc<PtyManager>,
        output: Arc<dyn OutputSink>,
        observer: Arc<dyn SupervisorObserver>,
        config: SupervisorConfig,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                store,
                runtimes,
                pty,
                output,
                observer,
                config,
                agents: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn agents(&self) -> MutexGuard<'_, HashMap<AgentId, Supervised>> {
        // Nenhum código entra em pânico segurando este lock com o mapa pela metade.
        self.shared
            .agents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn state(&self, agent_id: &AgentId) -> AgentState {
        self.agents()
            .get(agent_id)
            .map_or(AgentState::Stopped, |s| s.state)
    }

    /// Sobe o agente. Um reinício agendado pela política é cancelado: quem pede
    /// explicitamente manda.
    pub async fn start(&self, agent_id: &AgentId) -> SupervisorResult<()> {
        if self.state(agent_id).is_running() {
            return Err(SupervisorError::AlreadyRunning(agent_id.clone()));
        }
        let store = &self.shared.store;
        let agent = store
            .get_agent(agent_id)
            .await?
            .ok_or_else(|| SupervisorError::AgentNotFound(agent_id.clone()))?;
        let team = store
            .get_team(&agent.team_id)
            .await?
            .ok_or_else(|| SupervisorError::TeamNotFound(agent.team_id.clone()))?;
        let adapter = self
            .shared
            .runtimes
            .adapter(&agent.adapter_id)
            .ok_or_else(|| SupervisorError::UnknownAdapter(agent.adapter_id.clone()))?;

        let token = generate_token()?;
        let plan = build_launch(
            &agent,
            &team,
            &adapter,
            &LaunchIdentity { token: &token },
            &self.shared.config.launch,
        );
        let plan = match plan {
            Ok(plan) => plan,
            Err(error) => {
                self.set_state(agent_id, AgentState::Failed);
                return Err(error.into());
            }
        };

        let session_id = SessionId::new();
        let log_path = self
            .shared
            .config
            .logs_dir
            .join(format!("{}.log", agent.id.as_str()));
        let (recorded_tx, recorded_rx) = watch::channel(false);

        // Registrado antes do spawn: um processo que morre na hora precisa achar a
        // sessão atual, senão o término seria descartado como de uma sessão antiga.
        {
            let mut agents = self.agents();
            let entry = agents
                .entry(agent_id.clone())
                .or_insert_with(Supervised::new);
            entry.cancel.cancel();
            entry.cancel = CancellationToken::new();
            entry.session = Some(session_id.clone());
            entry.started = Instant::now();
            entry.stop_requested = false;
            entry.restart_policy = agent.restart_policy;
            entry.state = AgentState::Starting;
        }
        self.notify(agent_id, AgentState::Starting);

        let spec = PtySpawn {
            command: plan.program.display().to_string(),
            args: plan.args,
            cwd: Some(plan.cwd),
            env: plan.env,
            size: self.shared.config.size,
            log_path: Some(log_path.clone()),
        };
        let hook = Arc::new(ExitHook {
            inner: Arc::clone(&self.shared.output),
            supervisor: Arc::downgrade(&self.shared),
            agent_id: agent_id.clone(),
            session_id: session_id.clone(),
            recorded: recorded_rx,
        });
        if let Err(error) = self.shared.pty.spawn(agent_id.as_str(), spec, hook) {
            self.set_state(agent_id, AgentState::Failed);
            return Err(error.into());
        }
        tracing::info!(agent = %agent_id, adapter = %adapter.id, "agente iniciado");

        let record = SessionRecord {
            id: session_id.clone(),
            agent_id: agent_id.clone(),
            pid: self.shared.pty.pid(agent_id.as_str()),
            started_at: now_ms(),
            ended_at: None,
            exit_code: None,
            log_path: log_path.display().to_string(),
        };
        if let Err(error) = store.start_session(&record).await {
            // O agente já está de pé; perder o histórico não é motivo para derrubá-lo.
            tracing::warn!(agent = %agent_id, %error, "sessão não registrada");
        }
        let _ = recorded_tx.send(true);

        // Até o detector de estado da Fase 03 existir, processo vivo = ocioso.
        let became_idle = {
            let mut agents = self.agents();
            match agents.get_mut(agent_id) {
                Some(entry)
                    if entry.session.as_ref() == Some(&session_id)
                        && entry.state == AgentState::Starting =>
                {
                    entry.state = AgentState::Idle;
                    true
                }
                _ => false,
            }
        };
        if became_idle {
            self.notify(agent_id, AgentState::Idle);
        }
        Ok(())
    }

    /// Para o agente e cancela qualquer reinício agendado. Parar quem já está parado
    /// não é erro.
    pub fn stop(&self, agent_id: &AgentId) -> SupervisorResult<()> {
        let running = {
            let mut agents = self.agents();
            match agents.get_mut(agent_id) {
                Some(entry) => {
                    entry.stop_requested = true;
                    entry.cancel.cancel();
                    entry.state.is_running()
                }
                None => false,
            }
        };
        if running {
            // O término chega pelo `ExitHook`, que marca `stopped`.
            match self.shared.pty.kill(agent_id.as_str()) {
                Ok(()) | Err(PtyError::Closed | PtyError::UnknownAgent(_)) => {}
                Err(error) => return Err(error.into()),
            }
        } else {
            self.set_state(agent_id, AgentState::Stopped);
        }
        Ok(())
    }

    /// Para, espera o processo sair e sobe de novo.
    pub async fn restart(&self, agent_id: &AgentId) -> SupervisorResult<()> {
        self.stop(agent_id)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.state(agent_id).is_running() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        self.start(agent_id).await
    }

    /// Cancela todos os reinícios agendados. Os processos são encerrados pelo dono
    /// do `PtyManager` (o app, ao fechar a janela).
    pub fn shutdown(&self) {
        let mut agents = self.agents();
        for entry in agents.values_mut() {
            entry.stop_requested = true;
            entry.cancel.cancel();
        }
    }

    async fn on_exit(
        &self,
        agent_id: AgentId,
        session_id: SessionId,
        code: i32,
        mut recorded: watch::Receiver<bool>,
    ) {
        let _ = tokio::time::timeout(SESSION_RECORD_WAIT, recorded.wait_for(|done| *done)).await;
        if let Err(error) = self
            .shared
            .store
            .end_session(&session_id, now_ms(), Some(code))
            .await
        {
            tracing::warn!(agent = %agent_id, %error, "fim da sessão não registrado");
        }

        let decision = {
            let mut agents = self.agents();
            let Some(entry) = agents.get_mut(&agent_id) else {
                return;
            };
            if entry.session.as_ref() != Some(&session_id) {
                return; // término de uma sessão que já foi substituída
            }
            let ran_for = entry.started.elapsed();
            let restart = !entry.stop_requested && entry.restart_policy.should_restart(Some(code));
            entry.state = if entry.stop_requested || code == 0 {
                AgentState::Stopped
            } else {
                AgentState::Failed
            };
            let delay = restart.then(|| entry.backoff.next_delay(ran_for));
            (entry.state, delay.map(|d| (d, entry.cancel.clone())))
        };
        let (state, restart) = decision;
        tracing::info!(agent = %agent_id, code, ?state, "processo do agente terminou");
        self.notify(&agent_id, state);

        if let Some((delay, cancel)) = restart {
            tracing::info!(agent = %agent_id, ?delay, "reinício agendado pela política");
            tokio::select! {
                () = tokio::time::sleep(delay) => {
                    if let Err(error) = self.start(&agent_id).await {
                        tracing::warn!(agent = %agent_id, %error, "reinício falhou");
                    }
                }
                () = cancel.cancelled() => {}
            }
        }
    }

    fn set_state(&self, agent_id: &AgentId, state: AgentState) {
        self.agents()
            .entry(agent_id.clone())
            .or_insert_with(Supervised::new)
            .state = state;
        self.notify(agent_id, state);
    }

    fn notify(&self, agent_id: &AgentId, state: AgentState) {
        self.shared.observer.state_changed(agent_id, state);
    }
}

/// Repassa a saída ao destino do app e avisa o supervisor quando o processo acaba.
struct ExitHook<S> {
    inner: Arc<dyn OutputSink>,
    /// Fraco: uma sessão viva não pode manter o supervisor vivo depois do app fechar.
    supervisor: Weak<Shared<S>>,
    agent_id: AgentId,
    session_id: SessionId,
    recorded: watch::Receiver<bool>,
}

impl<S: SupervisorStore> OutputSink for ExitHook<S> {
    fn data(&self, agent_id: &str, chunk: Vec<u8>) {
        self.inner.data(agent_id, chunk);
    }

    fn exit(&self, agent_id: &str, code: i32) {
        self.inner.exit(agent_id, code);
        let Some(shared) = self.supervisor.upgrade() else {
            return;
        };
        let supervisor = AgentSupervisor { shared };
        let (id, session, recorded) = (
            self.agent_id.clone(),
            self.session_id.clone(),
            self.recorded.clone(),
        );
        tokio::spawn(async move {
            supervisor.on_exit(id, session, code, recorded).await;
        });
    }
}

#[cfg(test)]
mod tests;
