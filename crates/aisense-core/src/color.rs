//! Paleta de identidade (`docs/08`, "Paleta de identidade dos agentes").

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Uma das oito matizes da paleta. O valor em OKLCH mora nos tokens CSS
/// (`--agent-<nome>`); o domínio só conhece o nome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum AgentColor {
    #[default]
    Violet,
    Cyan,
    Emerald,
    Amber,
    Rose,
    Indigo,
    Teal,
    Fuchsia,
}

impl AgentColor {
    /// Ordem de atribuição automática.
    pub const ALL: [AgentColor; 8] = [
        Self::Violet,
        Self::Cyan,
        Self::Emerald,
        Self::Amber,
        Self::Rose,
        Self::Indigo,
        Self::Teal,
        Self::Fuchsia,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Violet => "violet",
            Self::Cyan => "cyan",
            Self::Emerald => "emerald",
            Self::Amber => "amber",
            Self::Rose => "rose",
            Self::Indigo => "indigo",
            Self::Teal => "teal",
            Self::Fuchsia => "fuchsia",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == raw)
    }

    /// Próxima cor para um agente novo: a primeira da paleta que ninguém usa.
    /// Com as oito ocupadas, a menos usada (empate → ordem da paleta), para o
    /// mosaico continuar equilibrado a partir do nono agente.
    pub fn next_free(used: &[AgentColor]) -> Self {
        let count = |c: AgentColor| used.iter().filter(|u| **u == c).count();
        Self::ALL
            .into_iter()
            .min_by_key(|c| count(*c))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigns_colors_in_palette_order() {
        assert_eq!(AgentColor::next_free(&[]), AgentColor::Violet);
        assert_eq!(
            AgentColor::next_free(&[AgentColor::Violet]),
            AgentColor::Cyan
        );
    }

    #[test]
    fn fills_gaps_left_by_removed_agents() {
        let used = [AgentColor::Violet, AgentColor::Emerald];
        assert_eq!(AgentColor::next_free(&used), AgentColor::Cyan);
    }

    #[test]
    fn keeps_balance_after_the_palette_is_exhausted() {
        let mut used = AgentColor::ALL.to_vec();
        used.push(AgentColor::Violet);
        assert_eq!(AgentColor::next_free(&used), AgentColor::Cyan);
    }

    #[test]
    fn round_trips_through_its_name() {
        for c in AgentColor::ALL {
            assert_eq!(AgentColor::parse(c.as_str()), Some(c));
        }
        assert_eq!(AgentColor::parse("red"), None);
    }
}
