//! Adaptadores de runtime (`docs/05`): TOMLs que ensinam o AISENSE a iniciar e
//! conversar com cada CLI de IA, sem recompilar o app.

mod calibrate;
mod catalog;
mod detect;
mod file;
mod model;
mod watch;

pub use calibrate::{save_state_rules, CalibrateError};
pub use catalog::{AdapterCatalog, Builtin, BUILTIN_ADAPTERS};
pub(crate) use detect::which_in_path;
pub use detect::{
    detect_runtime, resolve_command, which, RuntimeInfo, RuntimeOverview, RuntimeRegistry,
    RuntimeStatus, AGENT_COMMAND_PLACEHOLDER, DETECT_TIMEOUT, SHELL_PLACEHOLDER,
};
pub use file::{parse_adapter, ADAPTER_ID_MAX, ADAPTER_NAME_MAX, QUIET_MS_RANGE};
pub use model::{
    Adapter, AdapterProblem, AdapterSource, Capabilities, DetectSpec, InjectMode, InjectRules,
    SkillsTarget, StateRules,
};
pub use watch::{AdapterWatchError, AdapterWatcher};
