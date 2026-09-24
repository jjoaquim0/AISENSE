//! aisense-core
#![forbid(unsafe_code)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

pub mod adapter;
pub mod agent;
mod app_info;
pub mod bench;
pub mod board;
pub mod bus;
mod color;
mod command_error;
pub mod fswatch;
mod ids;
pub mod notes;
pub mod notify;
mod paths;
pub mod project;
pub mod proposal;
pub mod repo;
pub mod settings;
pub mod skill;
pub mod state;
pub mod supervisor;
pub mod team;
mod time;
mod toml_pos;
pub mod transcript;
mod validation;

pub use app_info::AppInfo;
pub use color::AgentColor;
pub use command_error::CommandError;
pub use ids::{
    ActivityId, AgentId, BoardId, CardId, ChannelId, ColumnId, CommentId, MessageId, ProposalId,
    SessionId, SkillId, TeamId,
};
pub use paths::DataDir;
pub use time::{now_ms, Millis};
pub use validation::ValidationError;

/// Versão do domínio, exposta pelo app e pela CLI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
