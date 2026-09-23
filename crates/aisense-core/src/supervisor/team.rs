//! Controles da equipe inteira (`docs/09`, T4: ▶ ⏸ ⟳; F03-06).
//!
//! Tudo é assíncrono e cede o runtime entre um agente e outro: iniciar seis agentes
//! nunca prende a interface, que acompanha o avanço pelos eventos de progresso.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AgentNotice, AgentStartFailure, AgentSupervisor, SupervisorResult, SupervisorStore,
    TeamStartReport,
};
use crate::agent::Agent;
use crate::ids::{AgentId, TeamId};

/// Espera entre um spawn e o próximo ao iniciar a equipe. Subir seis CLIs de IA de
/// uma vez disputa CPU e disco na hora mais pesada de cada uma (carregar, ler o
/// projeto); escalonar deixa cada uma chegar ao prompt mais rápido.
pub const TEAM_START_STAGGER: Duration = Duration::from_millis(300);

/// Quanto "parar tudo" espera os processos saírem antes de desistir de esperar.
const STOP_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum TeamOp {
    Start,
    Stop,
    Restart,
}

/// Evento `team:progress`: quantos agentes a operação já tratou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct TeamProgress {
    pub team_id: TeamId,
    pub op: TeamOp,
    pub done: u32,
    pub total: u32,
    /// O agente sendo tratado agora (`None` no início e no fim).
    pub agent_id: Option<AgentId>,
    pub finished: bool,
}

/// Quem acompanha o avanço (o app emite `team:progress`).
pub type ProgressFn<'a> = &'a (dyn Fn(TeamProgress) + Send + Sync);

impl<S: SupervisorStore> AgentSupervisor<S> {
    /// ▶ Iniciar equipe: os agentes com `autostart` que não estão rodando, na ordem da
    /// equipe, com `stagger` entre um e outro. Quem falha não interrompe os demais.
    pub async fn start_team(
        &self,
        team_id: &TeamId,
        stagger: Duration,
        progress: ProgressFn<'_>,
    ) -> SupervisorResult<TeamStartReport> {
        let targets: Vec<Agent> = self
            .team_agents(team_id)
            .await?
            .into_iter()
            .filter(|a| a.autostart && !self.state(&a.id).is_running())
            .collect();
        Ok(self
            .start_many(team_id, TeamOp::Start, &targets, stagger, progress)
            .await)
    }

    /// ⏸ Parar tudo: para quem está rodando e espera os processos saírem.
    pub async fn stop_team(
        &self,
        team_id: &TeamId,
        progress: ProgressFn<'_>,
    ) -> SupervisorResult<()> {
        let targets: Vec<Agent> = self
            .team_agents(team_id)
            .await?
            .into_iter()
            .filter(|a| self.state(&a.id).is_running())
            .collect();
        self.stop_many(team_id, TeamOp::Stop, &targets, progress)
            .await;
        progress(self.progress(
            team_id,
            TeamOp::Stop,
            targets.len(),
            targets.len(),
            None,
            true,
        ));
        Ok(())
    }

    /// ⟳ Reiniciar tudo: para quem está rodando e sobe de novo, escalonado. Sem
    /// ninguém rodando, é o mesmo que iniciar a equipe.
    pub async fn restart_team(
        &self,
        team_id: &TeamId,
        stagger: Duration,
        progress: ProgressFn<'_>,
    ) -> SupervisorResult<TeamStartReport> {
        let agents = self.team_agents(team_id).await?;
        let running: Vec<Agent> = agents
            .iter()
            .filter(|a| self.state(&a.id).is_running())
            .cloned()
            .collect();
        if running.is_empty() {
            return self.start_team(team_id, stagger, progress).await;
        }
        self.stop_many(team_id, TeamOp::Restart, &running, &|_| {})
            .await;
        Ok(self
            .start_many(team_id, TeamOp::Restart, &running, stagger, progress)
            .await)
    }

    async fn team_agents(&self, team_id: &TeamId) -> SupervisorResult<Vec<Agent>> {
        // Já vem na ordem da equipe (`position`).
        Ok(self.shared.store.list_agents(team_id).await?)
    }

    async fn start_many(
        &self,
        team_id: &TeamId,
        op: TeamOp,
        targets: &[Agent],
        stagger: Duration,
        progress: ProgressFn<'_>,
    ) -> TeamStartReport {
        let total = targets.len();
        let mut report = TeamStartReport::default();
        progress(self.progress(team_id, op, 0, total, None, false));
        for (index, agent) in targets.iter().enumerate() {
            if index > 0 {
                tokio::time::sleep(stagger).await;
            }
            progress(self.progress(team_id, op, index, total, Some(&agent.id), false));
            match self.start(&agent.id).await {
                Ok(outcome) => {
                    if let Some(message) = outcome.workdir.warning {
                        report.notices.push(AgentNotice {
                            agent_id: agent.id.clone(),
                            message,
                        });
                    }
                }
                Err(error) => report.failures.push(AgentStartFailure {
                    agent_id: agent.id.clone(),
                    error: error.to_command_error(),
                }),
            }
        }
        progress(self.progress(team_id, op, total, total, None, true));
        report
    }

    async fn stop_many(
        &self,
        team_id: &TeamId,
        op: TeamOp,
        targets: &[Agent],
        progress: ProgressFn<'_>,
    ) {
        let total = targets.len();
        for (index, agent) in targets.iter().enumerate() {
            progress(self.progress(team_id, op, index, total, Some(&agent.id), false));
            if let Err(error) = self.stop(&agent.id) {
                tracing::warn!(agent = %agent.id, %error, "agente não parou");
            }
        }
        // Subir de novo antes de o processo antigo sair esbarraria em `AlreadyRunning`.
        let deadline = Instant::now() + STOP_WAIT;
        while targets.iter().any(|a| self.state(&a.id).is_running()) && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn progress(
        &self,
        team_id: &TeamId,
        op: TeamOp,
        done: usize,
        total: usize,
        agent_id: Option<&AgentId>,
        finished: bool,
    ) -> TeamProgress {
        TeamProgress {
            team_id: team_id.clone(),
            op,
            done: u32::try_from(done).unwrap_or(u32::MAX),
            total: u32::try_from(total).unwrap_or(u32::MAX),
            agent_id: agent_id.cloned(),
            finished,
        }
    }
}
