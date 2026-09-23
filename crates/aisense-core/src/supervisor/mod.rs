//! Supervisor de agentes (`docs/02`, fluxo 1 e "Máquina de estados do agente").
//!
//! Sobe o processo de cada agente num PTY com o comando e o ambiente certos, registra
//! a sessão, e quando o processo termina decide — pela política do agente — se ele
//! volta, com espera crescente entre quedas seguidas.

mod backoff;
mod launch;
mod team;
mod token;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use aisense_pty::{OutputSink, PtyError, PtyManager, PtySpawn, TerminalSize};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

pub use backoff::{Backoff, FIRST_DELAY, MAX_DELAY, STABLE_AFTER};
pub use launch::{build_launch, LaunchContext, LaunchError, LaunchIdentity, LaunchPlan};
pub use team::{ProgressFn, TeamOp, TeamProgress, TEAM_START_STAGGER};
pub use token::{generate_token, TokenError};

use crate::adapter::RuntimeRegistry;
use crate::agent::{AgentState, RestartPolicy};
use crate::bench::{prepare_workdir, remove_bench, BenchError, Workdir};
use crate::ids::{AgentId, SessionId, TeamId};
use crate::project::{load_project, ProjectLookup};
use crate::repo::{
    AgentRepository, RepoError, SessionRecord, SessionRepository, SkillRepository, TeamRepository,
};
use crate::skill::{
    materialize, resolve_agent_skills, MaterializeRequest, SkillLibrary, SkillPlan,
};
use crate::state::{Detection, StateConfidence, StateDetector};
use crate::time::now_ms;

/// Quanto o término de uma sessão espera o registro do início dela. Só importa
/// quando o processo morre antes de o supervisor terminar de gravar o início.
const SESSION_RECORD_WAIT: Duration = Duration::from_secs(5);

/// Payload do evento `agent:state` que o app emite a cada mudança.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentStateChanged {
    pub agent_id: AgentId,
    pub state: AgentState,
    /// `low` quando o estado veio só do silêncio, sem regex casando (`docs/05`).
    pub confidence: StateConfidence,
}

/// O que um start decidiu: onde o agente trabalha e alguma ressalva para a UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct StartOutcome {
    pub workdir: Workdir,
    /// Skills que o agente levou e as que ficaram de fora, com o porquê (F04-03).
    pub skills: SkillPlan,
    /// Outras ressalvas do boot, prontas para a UI (materialização que falhou, skill
    /// nativa que já existia e não é nossa — F04-04).
    pub notes: Vec<String>,
}

/// Uma ressalva sobre um agente que subiu (sem git, setup que falhou...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentNotice {
    pub agent_id: AgentId,
    pub message: String,
}

/// Resultado de "▶ Iniciar equipe": quem não subiu e quem subiu com ressalva.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct TeamStartReport {
    pub failures: Vec<AgentStartFailure>,
    pub notices: Vec<AgentNotice>,
}

/// Um agente que não subiu ao iniciar a equipe inteira (T2, "▶ Iniciar equipe").
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentStartFailure {
    pub agent_id: AgentId,
    pub error: crate::CommandError,
}

/// Quem precisa saber das mudanças de estado (o app emite `agent:state`).
pub trait SupervisorObserver: Send + Sync + 'static {
    fn state_changed(&self, agent_id: &AgentId, state: AgentState, confidence: StateConfidence);
}

/// Miniatura de um agente: as últimas linhas da tela, sem ANSI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentPreview {
    pub agent_id: AgentId,
    pub lines: Vec<String>,
}

/// Teto de linhas por miniatura: é uma prévia, não um segundo terminal.
pub const MAX_PREVIEW_LINES: usize = 20;

/// O que chega à tarefa do detector de estado de uma sessão.
enum DetectorInput {
    Output(Vec<u8>),
    Resize(TerminalSize),
}

/// As portas de que o supervisor precisa, juntas.
pub trait SupervisorStore:
    TeamRepository + AgentRepository + SessionRepository + SkillRepository + 'static
{
}
impl<T: TeamRepository + AgentRepository + SessionRepository + SkillRepository + 'static>
    SupervisorStore for T
{
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// `logs/<agent_id>.log` (`docs/02`, "Persistência e layout em disco").
    pub logs_dir: PathBuf,
    /// `benches/` (`docs/16`): onde nascem os worktrees das bancadas.
    pub benches_dir: PathBuf,
    pub launch: LaunchContext,
    /// Tamanho inicial do terminal; a UI redimensiona quando o painel abre.
    pub size: TerminalSize,
    /// Biblioteca de skills atual: o start resolve as do agente contra ela (F04-03).
    pub skills: Arc<SkillLibrary>,
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
    #[error(transparent)]
    Bench(#[from] BenchError),
}

impl SupervisorError {
    /// Forma que a interface recebe.
    pub fn to_command_error(&self) -> crate::CommandError {
        crate::CommandError::new(self.code(), self.to_string(), self.hint())
    }

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
            Self::Bench(e) => e.code(),
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
            Self::Bench(BenchError::Dirty { .. }) => Some(
                "Commite ou descarte as mudanças na bancada do agente antes de continuar."
                    .to_owned(),
            ),
            Self::Bench(BenchError::Occupied(_)) => {
                Some("Mova ou apague essa pasta; ela não é uma bancada do AISENSE.".to_owned())
            }
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
    /// Canal para o detector da sessão atual (redimensionamento). `None` quando não
    /// há processo.
    detector: Option<mpsc::UnboundedSender<DetectorInput>>,
    /// A tela da sessão mais recente, compartilhada com a tarefa do detector. Fica
    /// depois que o processo morre: a miniatura de um agente que caiu mostra o erro.
    screen: Option<Arc<Mutex<StateDetector>>>,
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
            detector: None,
            screen: None,
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
    pub async fn start(&self, agent_id: &AgentId) -> SupervisorResult<StartOutcome> {
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
        let colleagues = store.list_agents(&team.id).await?;
        let adapter = self
            .shared
            .runtimes
            .adapter(&agent.adapter_id)
            .ok_or_else(|| SupervisorError::UnknownAdapter(agent.adapter_id.clone()))?;

        // Skills: as habilitadas, no disco e compatíveis com o runtime (`docs/06`). As que
        // ficam de fora não impedem o start — voltam no resultado para a UI avisar.
        let skills = resolve_agent_skills(
            &**store,
            &self.shared.config.skills.catalog(),
            agent_id,
            &agent.adapter_id,
        )
        .await?;
        for ignored in &skills.ignored {
            tracing::warn!(agent = %agent_id, %ignored, "skill ignorada no boot");
        }

        // Bancada: pode criar worktree, copiar arquivos e rodar o setup — tudo
        // bloqueante, então fora das threads do runtime assíncrono.
        let workdir = match self.prepare_workdir(&team, &agent).await {
            Ok(workdir) => workdir,
            Err(error) => {
                self.set_state(agent_id, AgentState::Failed);
                return Err(error);
            }
        };
        if let Some(warning) = &workdir.warning {
            tracing::warn!(agent = %agent_id, %warning, "diretório de trabalho com ressalva");
        }

        // Materializa skills, identidade e BOOT.md no diretório de trabalho
        // (F04-04/05). Falha de disco vira ressalva, não impede o start.
        let notes = self
            .materialize(&team, &agent, &colleagues, &adapter, &workdir, &skills.active)
            .await;

        let token = generate_token()?;
        let plan = build_launch(
            &agent,
            &team,
            &adapter,
            &LaunchIdentity { token: &token },
            &self.shared.config.launch,
            std::path::Path::new(&workdir.path),
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
        // A sessão nova começa onde o log está agora: a anterior já saiu e drenou (F03-09).
        let log_offset = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
        let (recorded_tx, recorded_rx) = watch::channel(false);
        // Criado antes do spawn, como o receptor principal do PTY: a saída do boot
        // fica no canal até a tarefa do detector começar a ler.
        let (detector_tx, detector_rx) = mpsc::unbounded_channel();

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
            entry.detector = Some(detector_tx.clone());
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
            detector: detector_tx,
        });
        // Nasce invisível: só um painel na tela liga os eventos (F03-05).
        if let Err(error) = self.shared.pty.spawn_hidden(agent_id.as_str(), spec, hook) {
            self.set_state(agent_id, AgentState::Failed);
            return Err(error.into());
        }
        tracing::info!(agent = %agent_id, adapter = %adapter.id, "agente iniciado");
        let size = self.shared.config.size;
        let detector = Arc::new(Mutex::new(StateDetector::new(
            &adapter.state,
            size.rows,
            size.cols,
            Instant::now(),
        )));
        if let Some(entry) = self.agents().get_mut(agent_id) {
            entry.screen = Some(Arc::clone(&detector));
        }
        tokio::spawn(run_detector(
            Arc::downgrade(&self.shared),
            agent_id.clone(),
            session_id.clone(),
            detector,
            detector_rx,
        ));

        let record = SessionRecord {
            id: session_id.clone(),
            agent_id: agent_id.clone(),
            pid: self.shared.pty.pid(agent_id.as_str()),
            started_at: now_ms(),
            ended_at: None,
            exit_code: None,
            log_path: log_path.display().to_string(),
            log_offset: Some(log_offset),
        };
        if let Err(error) = store.start_session(&record).await {
            // O agente já está de pé; perder o histórico não é motivo para derrubá-lo.
            tracing::warn!(agent = %agent_id, %error, "sessão não registrada");
        }
        let _ = recorded_tx.send(true);

        // Fica em `starting` até o detector ler a primeira tela (F03-01).
        Ok(StartOutcome {
            workdir,
            skills: skills.plan(),
            notes,
        })
    }

    async fn materialize(
        &self,
        team: &crate::team::Team,
        agent: &crate::agent::Agent,
        colleagues: &[crate::agent::Agent],
        adapter: &crate::adapter::Adapter,
        workdir: &Workdir,
        skills: &[crate::skill::Skill],
    ) -> Vec<String> {
        let (team, agent, colleagues, adapter) =
            (team.clone(), agent.clone(), colleagues.to_vec(), adapter.clone());
        let (path, skills) = (PathBuf::from(&workdir.path), skills.to_vec());
        let agent_id = agent.id.clone();
        let done = tokio::task::spawn_blocking(move || {
            materialize(&MaterializeRequest {
                workdir: &path,
                agent: &agent,
                team: &team,
                colleagues: &colleagues,
                adapter: &adapter,
                skills: &skills,
                now: now_ms(),
            })
        })
        .await;
        match done {
            Ok(Ok(done)) => done.warnings,
            Ok(Err(error)) => {
                tracing::warn!(agent = %agent_id, %error, "boot não materializado");
                vec![format!(
                    "o boot do agente não foi preparado completamente: {error}"
                )]
            }
            Err(error) => {
                tracing::warn!(agent = %agent_id, %error, "materialização interrompida");
                vec!["o boot do agente não foi preparado completamente".into()]
            }
        }
    }

    async fn prepare_workdir(
        &self,
        team: &crate::team::Team,
        agent: &crate::agent::Agent,
    ) -> SupervisorResult<Workdir> {
        let benches = self.shared.config.benches_dir.clone();
        let (team, agent) = (team.clone(), agent.clone());
        tokio::task::spawn_blocking(move || {
            let project = match load_project(std::path::Path::new(&team.workdir)) {
                ProjectLookup::Found { config, .. } => Some(config),
                _ => None,
            };
            prepare_workdir(&team, &agent, &benches, project.as_ref())
        })
        .await
        .map_err(|e| SupervisorError::Bench(BenchError::Io(e.to_string())))?
        .map_err(SupervisorError::from)
    }

    /// Para o agente e remove a bancada dele, se houver. **Recusa** com mudanças não
    /// commitadas na bancada — usado antes de excluir o agente (`docs/16`).
    pub async fn retire(&self, agent_id: &AgentId) -> SupervisorResult<()> {
        let store = &self.shared.store;
        let agent = store
            .get_agent(agent_id)
            .await?
            .ok_or_else(|| SupervisorError::AgentNotFound(agent_id.clone()))?;
        let team = store
            .get_team(&agent.team_id)
            .await?
            .ok_or_else(|| SupervisorError::TeamNotFound(agent.team_id.clone()))?;
        self.stop(agent_id)?;
        let benches = self.shared.config.benches_dir.clone();
        tokio::task::spawn_blocking(move || remove_bench(&benches, &team, &agent))
            .await
            .map_err(|e| SupervisorError::Bench(BenchError::Io(e.to_string())))??;
        Ok(())
    }

    pub fn benches_dir(&self) -> &std::path::Path {
        &self.shared.config.benches_dir
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
    pub async fn restart(&self, agent_id: &AgentId) -> SupervisorResult<StartOutcome> {
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
            entry.detector = None;
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
        // Transições do supervisor são fatos (subiu, morreu, parou), não heurística.
        self.shared
            .observer
            .state_changed(agent_id, state, StateConfidence::High);
    }

    /// Últimas linhas da tela de cada agente pedido (vista Foco, F03-04). Agente que
    /// nunca subiu nesta execução do app não aparece na resposta.
    pub fn previews(&self, agent_ids: &[AgentId], lines: usize) -> Vec<AgentPreview> {
        let lines = lines.clamp(1, MAX_PREVIEW_LINES);
        let screens: Vec<(AgentId, Arc<Mutex<StateDetector>>)> = {
            let agents = self.agents();
            agent_ids
                .iter()
                .filter_map(|id| Some((id.clone(), agents.get(id)?.screen.clone()?)))
                .collect()
        };
        screens
            .into_iter()
            .map(|(agent_id, screen)| AgentPreview {
                agent_id,
                lines: screen
                    .lock()
                    .map(|d| d.last_lines(lines))
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// O painel do agente mudou de tamanho: a tela do detector precisa acompanhar,
    /// senão uma TUI desenhada para outra largura vira texto quebrado.
    pub fn resized(&self, agent_id: &AgentId, size: TerminalSize) {
        if let Some(tx) = self.agents().get(agent_id).and_then(|e| e.detector.clone()) {
            let _ = tx.send(DetectorInput::Resize(size));
        }
    }

    /// Aplica uma decisão do detector. `false` = a sessão já não é a atual (ou o
    /// processo morreu) e o detector deve parar.
    fn apply_detection(
        &self,
        agent_id: &AgentId,
        session_id: &SessionId,
        detection: Detection,
    ) -> bool {
        {
            let mut agents = self.agents();
            let Some(entry) = agents.get_mut(agent_id) else {
                return false;
            };
            if entry.session.as_ref() != Some(session_id) || !entry.state.is_running() {
                return false;
            }
            entry.state = detection.state;
        }
        tracing::debug!(agent = %agent_id, state = ?detection.state, confidence = ?detection.confidence, "estado do agente");
        self.shared
            .observer
            .state_changed(agent_id, detection.state, detection.confidence);
        true
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
    detector: mpsc::UnboundedSender<DetectorInput>,
}

impl<S: SupervisorStore> OutputSink for ExitHook<S> {
    fn data(&self, agent_id: &str, chunk: Vec<u8>) {
        self.inner.data(agent_id, chunk);
    }

    fn raw(&self, _agent_id: &str, chunk: &[u8]) {
        // Detector já encerrado (sessão substituída) não é erro.
        let _ = self.detector.send(DetectorInput::Output(chunk.to_vec()));
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

/// Uma tarefa por sessão: alimenta o detector com a saída e acorda nos prazos que
/// ele pede. Termina quando a sessão acaba ou é substituída.
async fn run_detector<S: SupervisorStore>(
    shared: Weak<Shared<S>>,
    agent_id: AgentId,
    session_id: SessionId,
    detector: Arc<Mutex<StateDetector>>,
    mut input: mpsc::UnboundedReceiver<DetectorInput>,
) {
    // O lock é curto e nunca atravessa um `await`: a miniatura lê a mesma tela.
    let with = |f: &mut dyn FnMut(&mut StateDetector) -> Option<Detection>| {
        detector.lock().ok().and_then(|mut d| f(&mut d))
    };
    loop {
        let deadline = detector.lock().ok().and_then(|d| d.next_deadline());
        let detection = tokio::select! {
            received = input.recv() => match received {
                Some(DetectorInput::Output(chunk)) => with(&mut |d| d.feed(&chunk, Instant::now())),
                Some(DetectorInput::Resize(size)) => with(&mut |d| {
                    d.resize(size.rows, size.cols);
                    None
                }),
                None => return,
            },
            () = sleep_until(deadline) => with(&mut |d| d.tick(Instant::now())),
        };
        if let Some(detection) = detection {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            let supervisor = AgentSupervisor { shared };
            if !supervisor.apply_detection(&agent_id, &session_id, detection) {
                return;
            }
        }
    }
}

/// Dorme até o prazo, ou para sempre quando não há prazo.
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(tokio::time::Instant::from_std(at)).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests;
