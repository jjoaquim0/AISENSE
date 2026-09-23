mod model;
mod setup;
mod templates;

pub use model::{Team, TeamDraft, TEAM_MISSION_MAX, TEAM_NAME_MAX};
pub use setup::{
    confirm_deletion, create_team_with_agents, AgentSummary, TeamSetupError, TeamSummary,
};
pub use templates::{PlannedAgent, TeamTemplate};
