//! Guardas anti-laço (`docs/07`, "Proteções contra laço infinito"; F05-10). Ligadas por
//! padrão. Valem só para mensagens de **agentes** — o humano e o próprio AISENSE passam.
//!
//! | Guarda | Padrão | Ao estourar |
//! |---|---|---|
//! | Taxa por agente | 30 msg/min | `rate_limited` |
//! | Profundidade de cadeia de reply | 12 | passa, com aviso de sistema ao remetente |
//! | Mensagem idêntica repetida | 3 | bloqueia a 4ª e avisa o humano |
//! | Orçamento da equipe | 500 msg/h | pausa a equipe até o humano liberar |

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AgentId, TeamId};
use crate::time::Millis;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct GuardConfig {
    pub per_agent_per_minute: u32,
    pub max_reply_depth: u32,
    pub max_identical: u32,
    pub team_per_hour: u32,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            per_agent_per_minute: 30,
            max_reply_depth: 12,
            max_identical: 3,
            team_per_hour: 500,
        }
    }
}

/// Por que uma mensagem foi barrada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum BlockReason {
    RateLimited { per_minute: u32 },
    Repeated { times: u32 },
    TeamBudget { per_hour: u32 },
}

/// Payload de `bus:blocked`: a UI mostra e oferece a ação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct BusBlocked {
    pub team_id: TeamId,
    pub agent_id: AgentId,
    pub reason: BlockReason,
    /// Frase pronta em pt-BR.
    pub detail: String,
}

const MINUTE: Millis = 60_000;
const HOUR: Millis = 60 * MINUTE;
/// Janela em que mensagens iguais contam como repetição.
const REPEAT_WINDOW: Millis = 10 * MINUTE;

#[derive(Default)]
pub(crate) struct GuardState {
    config: GuardConfig,
    per_agent: HashMap<AgentId, VecDeque<Millis>>,
    per_team: HashMap<TeamId, VecDeque<Millis>>,
    /// (remetente, destino, corpo) → quantas vezes seguidas, e quando foi a última.
    repeats: HashMap<(AgentId, String, String), (u32, Millis)>,
    /// Equipes pausadas pelo orçamento até o humano liberar.
    paused: HashMap<TeamId, ()>,
    /// Bloqueios já avisados ao humano, por agente: repetir o aviso a cada tentativa
    /// barrada viraria o próprio laço que a guarda existe para evitar.
    announced: HashMap<AgentId, std::mem::Discriminant<BlockReason>>,
    /// A pausa da equipe é uma só, seja qual for o agente barrado.
    announced_pause: std::collections::HashSet<TeamId>,
}

/// O que a checagem decidiu para uma mensagem de agente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    Pass,
    Block(BlockReason),
}

impl GuardState {
    pub fn with_config(config: GuardConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn config(&self) -> &GuardConfig {
        &self.config
    }

    /// Checa e, se passar, conta a mensagem.
    pub fn admit(
        &mut self,
        team: &TeamId,
        agent: &AgentId,
        to: &str,
        body: &str,
        now: Millis,
    ) -> Verdict {
        let config = self.config.clone();
        if self.paused.contains_key(team) {
            return Verdict::Block(BlockReason::TeamBudget {
                per_hour: config.team_per_hour,
            });
        }

        let mine = self.per_agent.entry(agent.clone()).or_default();
        while mine.front().is_some_and(|t| now - t >= MINUTE) {
            mine.pop_front();
        }
        if mine.len() as u32 >= config.per_agent_per_minute {
            return Verdict::Block(BlockReason::RateLimited {
                per_minute: config.per_agent_per_minute,
            });
        }

        let key = (agent.clone(), to.to_owned(), normalize(body));
        let repeated = match self.repeats.get(&key) {
            Some((n, last)) if now - last < REPEAT_WINDOW => *n,
            _ => 0,
        };
        if repeated >= config.max_identical {
            return Verdict::Block(BlockReason::Repeated {
                times: repeated + 1,
            });
        }

        let team_log = self.per_team.entry(team.clone()).or_default();
        while team_log.front().is_some_and(|t| now - t >= HOUR) {
            team_log.pop_front();
        }
        if team_log.len() as u32 >= config.team_per_hour {
            self.paused.insert(team.clone(), ());
            return Verdict::Block(BlockReason::TeamBudget {
                per_hour: config.team_per_hour,
            });
        }

        // Passou: conta.
        self.announced.remove(agent);
        let mine = self.per_agent.entry(agent.clone()).or_default();
        mine.push_back(now);
        self.per_team
            .entry(team.clone())
            .or_default()
            .push_back(now);
        // Qualquer outra mensagem do mesmo agente zera as repetições dele.
        self.repeats.retain(|(a, _, _), _| a != agent);
        self.repeats.insert(key, (repeated + 1, now));
        Verdict::Pass
    }

    /// O humano mandou continuar: tira a pausa e zera o orçamento da hora.
    pub fn resume(&mut self, team: &TeamId) {
        self.paused.remove(team);
        self.per_team.remove(team);
        self.announced.clear();
        self.announced_pause.remove(team);
    }

    /// `true` na primeira vez que este bloqueio acontece para o agente (desde a última
    /// mensagem que passou): só então o humano é avisado.
    pub fn first_block(&mut self, team: &TeamId, agent: &AgentId, reason: &BlockReason) -> bool {
        if matches!(reason, BlockReason::TeamBudget { .. }) {
            return self.announced_pause.insert(team.clone());
        }
        let kind = std::mem::discriminant(reason);
        self.announced.insert(agent.clone(), kind) != Some(kind)
    }

    pub fn is_paused(&self, team: &TeamId) -> bool {
        self.paused.contains_key(team)
    }
}

/// Espaços a mais não tornam uma mensagem diferente.
fn normalize(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl BlockReason {
    pub fn describe(&self, handle: &str) -> String {
        match self {
            Self::RateLimited { per_minute } => format!(
                "@{handle} passou de {per_minute} mensagens por minuto; mensagens barradas até a taxa baixar"
            ),
            Self::Repeated { times } => format!(
                "@{handle} tentou mandar a mesma mensagem pela {times}ª vez; barrada para evitar laço"
            ),
            Self::TeamBudget { per_hour } => format!(
                "a equipe passou de {per_hour} mensagens na última hora; entregas pausadas até você liberar"
            ),
        }
    }

    pub fn hint(&self) -> String {
        match self {
            Self::RateLimited { .. } => {
                "Agrupe o que tem a dizer numa mensagem só e tente em um minuto.".into()
            }
            Self::Repeated { .. } => {
                "Você já mandou isso. Se não houve avanço, registre o impasse com aisense note e siga."
                    .into()
            }
            Self::TeamBudget { .. } => {
                "A equipe está pausada; o humano decide se continua. Siga trabalhando sem mensagens."
                    .into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (TeamId, AgentId, AgentId) {
        (TeamId::new(), AgentId::new(), AgentId::new())
    }

    #[test]
    fn taxa_por_agente_por_minuto() {
        let (team, a, b) = ids();
        let mut g = GuardState::with_config(GuardConfig {
            per_agent_per_minute: 3,
            ..GuardConfig::default()
        });
        for i in 0..3 {
            assert_eq!(g.admit(&team, &a, "@b", &format!("m{i}"), i), Verdict::Pass);
        }
        assert!(matches!(
            g.admit(&team, &a, "@b", "m3", 10),
            Verdict::Block(BlockReason::RateLimited { per_minute: 3 })
        ));
        assert_eq!(g.admit(&team, &b, "@a", "outro agente", 10), Verdict::Pass);
        // Um minuto depois, libera.
        assert_eq!(g.admit(&team, &a, "@b", "m4", MINUTE + 1), Verdict::Pass);
    }

    #[test]
    fn quarta_mensagem_identica_e_barrada() {
        let (team, a, _) = ids();
        let mut g = GuardState::default();
        for t in 0..3 {
            assert_eq!(g.admit(&team, &a, "@b", "tudo  certo?", t), Verdict::Pass);
        }
        assert!(matches!(
            g.admit(&team, &a, "@b", "tudo certo?", 4),
            Verdict::Block(BlockReason::Repeated { times: 4 })
        ));
        // Para outro destino, ou depois de mandar outra coisa, conta de novo.
        assert_eq!(g.admit(&team, &a, "@c", "tudo certo?", 5), Verdict::Pass);
        assert_eq!(g.admit(&team, &a, "@b", "tudo certo?", 6), Verdict::Pass);
    }

    #[test]
    fn orcamento_da_equipe_pausa_ate_o_humano_liberar() {
        let (team, a, b) = ids();
        let mut g = GuardState::with_config(GuardConfig {
            team_per_hour: 4,
            ..GuardConfig::default()
        });
        for i in 0..4 {
            let who = if i % 2 == 0 { &a } else { &b };
            assert_eq!(g.admit(&team, who, "@x", &format!("{i}"), i), Verdict::Pass);
        }
        assert!(matches!(
            g.admit(&team, &a, "@x", "5", 5),
            Verdict::Block(BlockReason::TeamBudget { .. })
        ));
        assert!(g.is_paused(&team));
        // Pausada, até o tempo passar não adianta: só o humano libera.
        assert!(matches!(
            g.admit(&team, &b, "@x", "6", 2 * HOUR),
            Verdict::Block(BlockReason::TeamBudget { .. })
        ));
        g.resume(&team);
        assert_eq!(g.admit(&team, &b, "@x", "7", 2 * HOUR), Verdict::Pass);
    }
}
