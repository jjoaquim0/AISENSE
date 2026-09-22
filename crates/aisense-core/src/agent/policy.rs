//! Políticas por agente. Cada enum espelha uma coluna `TEXT` de `agents`
//! (`docs/04`) e tem `as_str`/`parse` para o store não precisar reinventar a
//! grafia.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $text:literal),+ $(,)? } default $default:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        #[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
        pub enum $name {
            $(
                $(#[$vmeta])*
                #[serde(rename = $text)]
                $variant,
            )+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }

            pub fn parse(raw: &str) -> Option<Self> {
                match raw {
                    $($text => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl std::default::Default for $name {
            fn default() -> Self {
                $name::$default
            }
        }
    };
}

text_enum! {
    /// Como mensagens do barramento chegam ao agente (`docs/07`, ADR 0006).
    DeliveryMode {
        /// Caixa de entrada; o agente busca quando quiser. Padrão seguro.
        Pull => "pull",
        /// Injeta no PTY quando o detector disser que está ocioso.
        Push => "push",
        /// O próprio runtime checa ao fim de cada turno (precisa de suporte a hooks).
        Hook => "hook",
    } default Pull
}

text_enum! {
    /// O que fazer quando o processo do agente termina.
    RestartPolicy {
        Never => "never",
        /// Só reinicia em saída com erro; um `exit 0` é respeitado.
        OnCrash => "on-crash",
        Always => "always",
    } default OnCrash
}

text_enum! {
    /// Quanto o agente pode fazer sem pedir confirmação (`docs/11`).
    Autonomy {
        Ask => "ask",
        Trusted => "trusted",
    } default Ask
}

text_enum! {
    /// Modo de bancada da equipe (`docs/16`).
    WorkspaceMode {
        Shared => "shared",
        PerAgent => "per-agent",
    } default Shared
}

text_enum! {
    /// Exceção de bancada por agente (`docs/16`).
    Workbench {
        /// Segue o `WorkspaceMode` da equipe.
        Inherit => "inherit",
        Own => "own",
        Shared => "shared",
    } default Inherit
}

impl RestartPolicy {
    /// Decide se um término com `exit_code` deve reiniciar. `None` = morto por sinal.
    pub fn should_restart(self, exit_code: Option<i32>) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::OnCrash => exit_code != Some(0),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn defaults_match_the_schema() {
        assert_eq!(DeliveryMode::default().as_str(), "pull");
        assert_eq!(RestartPolicy::default().as_str(), "on-crash");
        assert_eq!(Autonomy::default().as_str(), "ask");
        assert_eq!(WorkspaceMode::default().as_str(), "shared");
        assert_eq!(Workbench::default().as_str(), "inherit");
    }

    #[test]
    fn text_and_serde_spelling_agree() {
        for p in RestartPolicy::ALL {
            let json = serde_json::to_string(p).unwrap();
            assert_eq!(json, format!("\"{}\"", p.as_str()));
            assert_eq!(RestartPolicy::parse(p.as_str()), Some(*p));
        }
        assert_eq!(
            WorkspaceMode::parse("per-agent"),
            Some(WorkspaceMode::PerAgent)
        );
        assert_eq!(DeliveryMode::parse("PUSH"), None);
    }

    #[test]
    fn restart_policy_decides_on_exit_code() {
        assert!(!RestartPolicy::Never.should_restart(Some(1)));
        assert!(!RestartPolicy::Never.should_restart(None));
        assert!(RestartPolicy::Always.should_restart(Some(0)));
        assert!(!RestartPolicy::OnCrash.should_restart(Some(0)));
        assert!(RestartPolicy::OnCrash.should_restart(Some(137)));
        assert!(RestartPolicy::OnCrash.should_restart(None));
    }
}
