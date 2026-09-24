//! Quando avisar o humano pelo SO (F08-07): um agente passou a esperar por ele ou caiu.
//!
//! A regra é pura para ser testada sem janela: quem chama informa se a janela está em
//! foco e qual equipe está na tela. O aviso nunca sai para a equipe que o usuário já está
//! olhando — ele vê o painel mudar de cor; o sistema tocando junto é ruído.

use crate::agent::AgentState;
use crate::ids::TeamId;
use crate::settings::NotificationSettings;
use crate::Millis;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyKind {
    AwaitingInput,
    Failed,
}

/// O que o app sabe da janela no instante da mudança.
#[derive(Debug, Clone, Copy)]
pub struct WindowContext<'a> {
    /// A janela do AISENSE tem o foco do SO.
    pub focused: bool,
    /// Equipe aberta na tela (Sala da Equipe); `None` na lista, skills ou configurações.
    pub viewing: Option<&'a TeamId>,
}

/// Avisar desta mudança de estado?
pub fn decide(
    prefs: &NotificationSettings,
    now: Millis,
    state: AgentState,
    agent_team: &TeamId,
    window: WindowContext<'_>,
) -> Option<NotifyKind> {
    let kind = match state {
        AgentState::AwaitingInput if prefs.awaiting_input => NotifyKind::AwaitingInput,
        AgentState::Failed if prefs.failed => NotifyKind::Failed,
        _ => return None,
    };
    if !prefs.enabled || prefs.muted(now) {
        return None;
    }
    if window.focused && window.viewing == Some(agent_team) {
        return None;
    }
    Some(kind)
}

/// Título e corpo do aviso, em pt-BR.
pub fn text(kind: NotifyKind, handle: &str, team_name: &str) -> (String, String) {
    match kind {
        NotifyKind::AwaitingInput => (
            format!("@{handle} está esperando por você"),
            format!("{team_name}: responda no terminal do agente."),
        ),
        NotifyKind::Failed => (
            format!("@{handle} caiu"),
            format!("{team_name}: veja o terminal do agente para o motivo."),
        ),
    }
}

/// Contagem para a bandeja: quantos esperam e quantos caíram, em todas as equipes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TraySummary {
    pub running: usize,
    pub awaiting: usize,
    pub failed: usize,
}

impl TraySummary {
    pub fn of(states: impl IntoIterator<Item = AgentState>) -> Self {
        states.into_iter().fold(Self::default(), |mut s, state| {
            match state {
                AgentState::AwaitingInput => s.awaiting += 1,
                AgentState::Failed => s.failed += 1,
                _ => {}
            }
            if state.is_running() {
                s.running += 1;
            }
            s
        })
    }

    /// Linha do menu e da dica da bandeja.
    pub fn line(&self) -> String {
        if self.running == 0 && self.failed == 0 {
            return "Nenhum agente rodando".to_owned();
        }
        let mut parts = vec![plural(self.running, "agente rodando", "agentes rodando")];
        if self.awaiting > 0 {
            parts.push(plural(self.awaiting, "esperando você", "esperando você"));
        }
        if self.failed > 0 {
            parts.push(plural(self.failed, "com falha", "com falha"));
        }
        parts.join(" · ")
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team(id: &str) -> TeamId {
        TeamId::from_raw(id)
    }

    fn window(focused: bool, viewing: Option<&TeamId>) -> WindowContext<'_> {
        WindowContext { focused, viewing }
    }

    #[test]
    fn only_awaiting_and_failed_notify() {
        let prefs = NotificationSettings::default();
        let a = team("team_a");
        for state in [
            AgentState::Idle,
            AgentState::Busy,
            AgentState::Starting,
            AgentState::Stopped,
        ] {
            assert_eq!(decide(&prefs, 0, state, &a, window(false, None)), None);
        }
        assert_eq!(
            decide(
                &prefs,
                0,
                AgentState::AwaitingInput,
                &a,
                window(false, None)
            ),
            Some(NotifyKind::AwaitingInput)
        );
        assert_eq!(
            decide(&prefs, 0, AgentState::Failed, &a, window(false, None)),
            Some(NotifyKind::Failed)
        );
    }

    #[test]
    fn never_for_the_team_on_screen_with_the_window_focused() {
        let prefs = NotificationSettings::default();
        let (a, b) = (team("team_a"), team("team_b"));
        let s = AgentState::AwaitingInput;
        // O aceite da F08-07.
        assert_eq!(decide(&prefs, 0, s, &a, window(true, Some(&a))), None);
        // Outra equipe na tela, ou a janela em segundo plano: avisa.
        assert!(decide(&prefs, 0, s, &a, window(true, Some(&b))).is_some());
        assert!(decide(&prefs, 0, s, &a, window(true, None)).is_some());
        assert!(decide(&prefs, 0, s, &a, window(false, Some(&a))).is_some());
    }

    #[test]
    fn respects_switches_and_mute() {
        let a = team("team_a");
        let off = NotificationSettings {
            enabled: false,
            ..NotificationSettings::default()
        };
        assert_eq!(
            decide(&off, 0, AgentState::Failed, &a, window(false, None)),
            None
        );
        let no_failed = NotificationSettings {
            failed: false,
            ..NotificationSettings::default()
        };
        assert_eq!(
            decide(&no_failed, 0, AgentState::Failed, &a, window(false, None)),
            None
        );
        assert!(decide(
            &no_failed,
            0,
            AgentState::AwaitingInput,
            &a,
            window(false, None)
        )
        .is_some());
        let muted = NotificationSettings {
            muted_until: Some(5_000),
            ..NotificationSettings::default()
        };
        assert_eq!(
            decide(&muted, 4_999, AgentState::Failed, &a, window(false, None)),
            None
        );
        assert!(decide(&muted, 5_000, AgentState::Failed, &a, window(false, None)).is_some());
    }

    #[test]
    fn tray_summary_counts_and_reads_well() {
        use AgentState::*;
        let s = TraySummary::of([Idle, Busy, AwaitingInput, Failed, Stopped]);
        assert_eq!(
            s,
            TraySummary {
                running: 3,
                awaiting: 1,
                failed: 1
            }
        );
        assert_eq!(
            s.line(),
            "3 agentes rodando · 1 esperando você · 1 com falha"
        );
        assert_eq!(TraySummary::of([Stopped]).line(), "Nenhum agente rodando");
        assert_eq!(TraySummary::of([Idle]).line(), "1 agente rodando");
    }

    #[test]
    fn text_names_the_agent_and_the_team() {
        let (title, body) = text(NotifyKind::AwaitingInput, "backend", "Squad");
        assert_eq!(title, "@backend está esperando por você");
        assert!(body.starts_with("Squad:"));
    }
}
