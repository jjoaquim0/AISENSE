//! aisense-core
#![forbid(unsafe_code)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

mod app_info;
mod command_error;
mod ids;
pub use app_info::AppInfo;
pub use command_error::CommandError;
pub use ids::{AgentId, BoardId, CardId, MessageId, SessionId, SkillId, TeamId};

/// Versão do domínio, exposta pelo app e pela CLI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
