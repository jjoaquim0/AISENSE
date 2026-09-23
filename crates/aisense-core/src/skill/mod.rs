//! Skills (`docs/06`): pacotes de instruções que o agente carrega ao nascer.
//!
//! Formato compatível com as skills do Claude Code (ADR 0005): um diretório com um
//! `SKILL.md` — frontmatter YAML + Markdown.

mod catalog;
mod library;
mod materialize;
mod model;
mod parse;
mod resolve;
mod watch;

pub use catalog::{BuiltinSkill, SkillCatalog, BUILTIN_SKILLS, SKILL_FILE};
pub use library::{SkillEntry, SkillLibrary, SkillLibraryView};
pub use materialize::{
    materialize, AgentCard, MaterializeError, MaterializeRequest, Materialized, AISENSE_DIR,
};
pub use model::{Skill, SkillInject, SkillProblem, SkillSource};
pub use parse::{parse_skill, SKILL_DESCRIPTION_MAX, SKILL_FILE_MAX_BYTES, SKILL_NAME_MAX};
pub use resolve::{resolve_agent_skills, IgnoreReason, IgnoredSkill, ResolvedSkills, SkillPlan};
pub use watch::SkillWatcher;
