//! Comandos do projeto: `aisense.toml` (`docs/17`).

mod config;
mod detect;

pub use config::{
    parse_project_config, BenchConfig, ConfigProblem, GatesConfig, ProjectCommand, ProjectConfig,
    DEFAULT_TIMEOUT_S, FILE_NAME, MAX_TIMEOUT_S,
};
pub use detect::{load_project, propose, ProjectLookup, Proposal};
