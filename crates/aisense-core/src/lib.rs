//! aisense-core
#![forbid(unsafe_code)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

pub mod agent;
mod app_info;
mod color;
mod command_error;
mod ids;
pub mod repo;
pub mod team;
mod time;
mod validation;

pub use app_info::AppInfo;
pub use color::AgentColor;
pub use command_error::CommandError;
pub use ids::{AgentId, BoardId, CardId, MessageId, SessionId, SkillId, TeamId};
pub use time::{now_ms, Millis};
pub use validation::ValidationError;

/// Versão do domínio, exposta pelo app e pela CLI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
