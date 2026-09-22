//! Máquina de estados do agente (`docs/02`, "Máquina de estados do agente").

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum AgentState {
    #[default]
    Stopped,
    Starting,
    Idle,
    Busy,
    AwaitingInput,
    Failed,
}

impl AgentState {
    /// Há um processo vivo por trás do agente.
    pub fn is_running(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Idle | Self::Busy | Self::AwaitingInput
        )
    }

    /// Só `idle` é seguro para escrever no PTY. Em `awaiting_input` a injeção
    /// responderia a uma pergunta que era para o humano (ADR 0006).
    pub fn accepts_injection(self) -> bool {
        self == Self::Idle
    }

    /// Transições permitidas pelo diagrama. Qualquer estado vivo pode cair para
    /// `stopped` (kill/exit) ou `failed` (crash).
    pub fn can_transition_to(self, next: Self) -> bool {
        use AgentState::*;
        match (self, next) {
            (Stopped | Failed, Starting) => true,
            (Starting, Idle) => true,
            (Idle, Busy) | (Busy, Idle) | (Busy, AwaitingInput) => true,
            (AwaitingInput, Busy | Idle) => true,
            (from, Stopped | Failed) => from.is_running(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::AgentState::*;

    #[test]
    fn only_idle_accepts_injection() {
        for s in [Stopped, Starting, Busy, AwaitingInput, Failed] {
            assert!(!s.accepts_injection(), "{s:?}");
        }
        assert!(Idle.accepts_injection());
    }

    #[test]
    fn follows_the_documented_diagram() {
        assert!(Stopped.can_transition_to(Starting));
        assert!(Starting.can_transition_to(Idle));
        assert!(Starting.can_transition_to(Failed));
        assert!(Busy.can_transition_to(AwaitingInput));
        assert!(Failed.can_transition_to(Starting), "restart policy");
        assert!(!Stopped.can_transition_to(Busy));
        assert!(!Stopped.can_transition_to(Failed));
        assert!(!Idle.can_transition_to(Starting));
    }

    #[test]
    fn serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&AwaitingInput).unwrap(),
            "\"awaiting_input\""
        );
    }
}
