//! Heurística que diz se o agente está ocioso, ocupado ou esperando o humano.
//!
//! As regras são as de `docs/05`:
//! 1. `idle` só com **silêncio** por `quiet_ms` **e** a tela casando com `idle_regex`;
//! 2. `awaiting_regex` tem prioridade máxima — entre linhas empatadas; quem decide
//!    primeiro é a linha mais baixa que casar (ver `evaluate`);
//! 3. silêncio total por 60 s sem regex nenhum casando vira `idle` com confiança baixa;
//! 4. o que se avalia é a **última tela**, sem ANSI — por isso um emulador de terminal
//!    (`vt100`) e não o log: CLIs de IA redesenham a tela com movimento de cursor, e o
//!    texto cru do log é uma sopa de fragmentos que nenhum regex casa com segurança.
//!
//! O detector é puro: recebe bytes e o instante atual e devolve decisões. Quem tem
//! relógio e timer é o supervisor. Assim os testes rodam transcrições inteiras em
//! microssegundos, com o tempo que quiserem.

use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::adapter::StateRules;
use crate::agent::AgentState;

/// Silêncio sem regex nenhum casando que faz o detector admitir `idle` (regra 3).
pub const LOW_CONFIDENCE_SILENCE: Duration = Duration::from_secs(60);

/// Quantas linhas não vazias do fim da tela os regex enxergam. Prompts e diálogos de
/// confirmação ficam no rodapé; olhar a tela inteira faria uma palavra como
/// "permission" no meio de uma resposta antiga marcar o agente como aguardando.
pub const TAIL_LINES: usize = 12;

/// Quanto dá para confiar na decisão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum StateConfidence {
    /// Um regex do adaptador casou (ou é um fato: saída chegando, processo morto).
    #[default]
    High,
    /// Nenhum regex casou; o estado veio só do silêncio. A entrega por `push` não
    /// deve confiar nisto (cai para `pull` naquela rodada, `docs/05`).
    Low,
}

/// Uma mudança de estado decidida pelo detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detection {
    pub state: AgentState,
    pub confidence: StateConfidence,
}

struct Rules {
    idle: Option<Regex>,
    busy: Option<Regex>,
    awaiting: Option<Regex>,
    quiet: Duration,
}

impl Rules {
    fn compile(rules: &StateRules) -> Self {
        Self {
            idle: compile("idle_regex", rules.idle_regex.as_deref()),
            busy: compile("busy_regex", rules.busy_regex.as_deref()),
            awaiting: compile("awaiting_regex", rules.awaiting_regex.as_deref()),
            quiet: Duration::from_millis(u64::from(rules.quiet_ms)),
        }
    }

    /// A linha mais baixa que casar decide; na mesma linha, aguardando > ocupado >
    /// ocioso. Devolve o estado e o índice da linha em `lines`.
    fn decide(&self, lines: &[&str]) -> Option<(AgentState, usize)> {
        let candidates = [
            (&self.awaiting, AgentState::AwaitingInput),
            (&self.busy, AgentState::Busy),
            (&self.idle, AgentState::Idle),
        ];
        lines.iter().enumerate().rev().find_map(|(at, line)| {
            candidates.iter().find_map(|(re, state)| {
                re.as_ref()
                    .is_some_and(|re| re.is_match(line))
                    .then_some((*state, at))
            })
        })
    }
}

pub struct StateDetector {
    screen: vt100::Parser,
    rules: Rules,
    state: AgentState,
    confidence: StateConfidence,
    /// Última saída recebida (ou o início da sessão, antes da primeira).
    last_output: Instant,
    /// A tela já foi avaliada depois da última saída?
    evaluated: bool,
    /// O recurso do silêncio longo já foi usado depois da última saída?
    fallback_used: bool,
}

impl StateDetector {
    /// Começa em `starting`. Os regex já foram validados na carga do adaptador; um
    /// que ainda assim não compile é tratado como ausente, com aviso.
    pub fn new(rules: &StateRules, rows: u16, cols: u16, now: Instant) -> Self {
        Self {
            screen: vt100::Parser::new(rows.max(1), cols.max(1), 0),
            rules: Rules::compile(rules),
            state: AgentState::Starting,
            confidence: StateConfidence::High,
            last_output: now,
            evaluated: false,
            fallback_used: false,
        }
    }

    pub fn state(&self) -> AgentState {
        self.state
    }

    /// Troca as regras sem reiniciar a sessão (modo calibração, F08-05). A tela atual é
    /// reavaliada assim que o silêncio mínimo das regras novas tiver passado — em geral
    /// no próximo `tick`, porque a tela já estava parada.
    pub fn set_rules(&mut self, rules: &StateRules) {
        self.rules = Rules::compile(rules);
        self.evaluated = false;
    }

    pub fn confidence(&self) -> StateConfidence {
        self.confidence
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.screen.screen_mut().set_size(rows.max(1), cols.max(1));
    }

    /// Saída do processo. Saída chegando é o sinal mais forte de "ocupado": um agente
    /// ocioso ou esperando resposta não escreve nada sozinho.
    pub fn feed(&mut self, bytes: &[u8], now: Instant) -> Option<Detection> {
        if bytes.is_empty() {
            return None;
        }
        self.screen.process(bytes);
        self.last_output = now;
        self.evaluated = false;
        self.fallback_used = false;
        match self.state {
            // Saída durante a subida é o próprio boot; continua `starting` até a
            // tela dizer algo.
            AgentState::Starting => None,
            AgentState::Idle | AgentState::AwaitingInput => {
                self.set(AgentState::Busy, StateConfidence::High)
            }
            _ => None,
        }
    }

    /// Avança o relógio. Chame em `next_deadline()` (ou depois, sem problema).
    pub fn tick(&mut self, now: Instant) -> Option<Detection> {
        let silent_for = now.saturating_duration_since(self.last_output);
        if !self.evaluated && silent_for >= self.rules.quiet {
            self.evaluated = true;
            if let Some(detection) = self.evaluate() {
                return Some(detection);
            }
        }
        if !self.fallback_used
            && silent_for >= LOW_CONFIDENCE_SILENCE
            && matches!(self.state, AgentState::Starting | AgentState::Busy)
        {
            self.fallback_used = true;
            return self.set(AgentState::Idle, StateConfidence::Low);
        }
        None
    }

    /// Quando `tick` pode ter algo a dizer. `None` = só depois de nova saída.
    pub fn next_deadline(&self) -> Option<Instant> {
        if !self.evaluated {
            return Some(self.last_output + self.rules.quiet);
        }
        let waiting = matches!(self.state, AgentState::Starting | AgentState::Busy);
        (waiting && !self.fallback_used).then(|| self.last_output + LOW_CONFIDENCE_SILENCE)
    }

    /// As últimas linhas não vazias da tela, sem ANSI — o que os regex enxergam.
    /// Serve também ao "modo calibração" da UI.
    pub fn screen_tail(&self) -> String {
        self.last_lines(TAIL_LINES).join("\n")
    }

    /// As `count` últimas linhas não vazias da tela, sem ANSI. É o que as miniaturas
    /// da vista Foco mostram: texto da tela de verdade, sem precisar de um xterm.
    pub fn last_lines(&self, count: usize) -> Vec<String> {
        let contents = self.screen.screen().contents();
        let lines: Vec<&str> = contents
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.trim().is_empty())
            .collect();
        let start = lines.len().saturating_sub(count);
        lines[start..].iter().map(|l| (*l).to_owned()).collect()
    }

    /// Decide pela **linha mais baixa** que casar com algum regex; na mesma linha,
    /// aguardando > ocupado > ocioso (regra 2).
    ///
    /// A posição importa porque a tela guarda o passado. Num programa de linha
    /// (shell, script), o "(s/n)" já respondido e o "compilando..." já terminado
    /// continuam visíveis acima do prompt novo; olhar só "casou em algum lugar"
    /// prenderia o agente em aguardando ou ocupado para sempre. O que está por
    /// último é o que o programa disse por último. CLIs com TUI redesenham a tela e
    /// trocam a caixa de entrada pelo diálogo, então nelas a regra dá o mesmo.
    fn evaluate(&mut self) -> Option<Detection> {
        let tail = self.screen_tail();
        let lines: Vec<&str> = tail.lines().collect();
        let decided = self.rules.decide(&lines).map(|(state, _)| state);
        match decided {
            Some(state) => self.set(state, StateConfidence::High),
            // Nada casou: quem estava ocupado continua (uma ferramenta demorada sem
            // saída); o silêncio longo decide depois.
            None => None,
        }
    }

    fn set(&mut self, state: AgentState, confidence: StateConfidence) -> Option<Detection> {
        if self.state == state && self.confidence == confidence {
            return None;
        }
        tracing::debug!(from = ?self.state, to = ?state, ?confidence, "estado detectado");
        self.state = state;
        self.confidence = confidence;
        Some(Detection { state, confidence })
    }
}

fn compile(field: &str, pattern: Option<&str>) -> Option<Regex> {
    let pattern = pattern?;
    match Regex::new(pattern) {
        Ok(re) => Some(re),
        Err(error) => {
            tracing::warn!(field, %error, "regex de estado inválido ignorado");
            None
        }
    }
}

/// Um regex do adaptador contra a tela, no modo calibração.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct PatternCheck {
    /// `idle_regex`, `busy_regex` ou `awaiting_regex`.
    pub field: String,
    /// Erro de compilação, pronto para mostrar. `None` quando o regex é válido ou vazio.
    pub error: Option<String>,
    /// Índices (em `lines`) das linhas que casam.
    pub matches: Vec<u32>,
}

/// Resultado do modo calibração (T9 → Runtimes; F08-05): o que o detector decidiria
/// para esta tela com estas regras, sem esperar o silêncio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Calibration {
    /// As linhas que os regex enxergam (as [`TAIL_LINES`] últimas não vazias).
    pub lines: Vec<String>,
    /// `None` quando nada casou: o detector manteria o estado e esperaria o silêncio longo.
    pub decided: Option<AgentState>,
    /// Linha que decidiu.
    pub decided_line: Option<u32>,
    pub patterns: Vec<PatternCheck>,
}

impl Calibration {
    /// Algum regex não compila? Não dá para aplicar assim.
    pub fn has_errors(&self) -> bool {
        self.patterns.iter().any(|p| p.error.is_some())
    }
}

/// Avalia `rules` contra `screen` exatamente como o detector faria (mesmas linhas,
/// mesma precedência), mas mostrando o porquê.
pub fn calibrate(rules: &StateRules, screen: &str) -> Calibration {
    let all: Vec<&str> = screen
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    let lines = &all[all.len().saturating_sub(TAIL_LINES)..];
    let fields = [
        ("awaiting_regex", rules.awaiting_regex.as_deref()),
        ("busy_regex", rules.busy_regex.as_deref()),
        ("idle_regex", rules.idle_regex.as_deref()),
    ];
    let patterns = fields
        .iter()
        .map(|(field, pattern)| {
            let pattern = pattern.filter(|p| !p.is_empty());
            let (error, matches) = match pattern.map(Regex::new) {
                None => (None, Vec::new()),
                Some(Err(e)) => (Some(e.to_string()), Vec::new()),
                Some(Ok(re)) => (
                    None,
                    (0u32..)
                        .zip(lines.iter())
                        .filter(|(_, l)| re.is_match(l))
                        .map(|(i, _)| i)
                        .collect(),
                ),
            };
            PatternCheck {
                field: (*field).to_owned(),
                error,
                matches,
            }
        })
        .collect();
    let decided = Rules::compile(rules).decide(lines);
    Calibration {
        lines: lines.iter().map(|l| (*l).to_owned()).collect(),
        decided: decided.map(|(state, _)| state),
        decided_line: decided.and_then(|(_, at)| u32::try_from(at).ok()),
        patterns,
    }
}

#[cfg(test)]
mod tests;
