//! Formato TOML do adaptador em disco e sua validação (`docs/05`, "Formato").

use std::collections::BTreeMap;

use serde::Deserialize;

use super::model::{
    Adapter, AdapterProblem, AdapterSource, Capabilities, DetectSpec, InjectMode, InjectRules,
    SkillsTarget, StateRules,
};
use crate::agent::RESERVED_ENV_PREFIX;

pub const ADAPTER_ID_MAX: usize = 64;
pub const ADAPTER_NAME_MAX: usize = 64;
pub const QUIET_MS_RANGE: std::ops::RangeInclusive<u32> = 50..=10_000;

const DEFAULT_QUIET_MS: u32 = 400;
const DEFAULT_SUBMIT: &str = "\r";
const DEFAULT_PREFIX: &str = "[AISENSE] ";
const DEFAULT_MAX_CHARS: u32 = 4_000;

// `deny_unknown_fields` em tudo: um erro de digitação (`idle_regx`) vira aviso com a
// linha, em vez de o campo ser ignorado e o detector de estado falhar em silêncio.

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterFile {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    icon: Option<String>,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    detect: Option<DetectFile>,
    install_hint: Option<String>,
    #[serde(default)]
    capabilities: CapabilitiesFile,
    #[serde(default)]
    state: StateFile,
    #[serde(default)]
    inject: InjectFile,
    skills: Option<SkillsFile>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DetectFile {
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct CapabilitiesFile {
    mcp: bool,
    system_prompt_flag: Option<String>,
    hooks: bool,
    model_flag: Option<String>,
    cwd_is_project: bool,
    resume_flag: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct StateFile {
    idle_regex: Option<String>,
    busy_regex: Option<String>,
    awaiting_regex: Option<String>,
    quiet_ms: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct InjectFile {
    mode: Option<InjectMode>,
    submit: Option<String>,
    prefix: Option<String>,
    max_chars: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillsFile {
    dir: String,
    format: Option<String>,
    settings_file: Option<String>,
}

/// Um campo recusado pela validação: `key` é o nome como aparece no TOML, usado
/// para achar a linha e apontá-la ao usuário.
struct Invalid {
    key: &'static str,
    message: String,
}

fn invalid(key: &'static str, message: impl Into<String>) -> Invalid {
    Invalid {
        key,
        message: message.into(),
    }
}

/// Lê e valida um adaptador. `path` só serve para compor a mensagem de erro.
pub fn parse_adapter(
    source: &str,
    path: &str,
    origin: AdapterSource,
) -> Result<Adapter, AdapterProblem> {
    let file: AdapterFile = toml::from_str(source).map_err(|err| {
        let (line, column) = err.span().map(|span| line_col(source, span.start)).unzip();
        AdapterProblem {
            path: path.to_owned(),
            line,
            column,
            message: err.message().trim().to_owned(),
        }
    })?;

    validate(file, origin).map_err(|bad| AdapterProblem {
        path: path.to_owned(),
        line: find_key_line(source, bad.key),
        column: None,
        message: bad.message,
    })
}

fn validate(file: AdapterFile, source: AdapterSource) -> Result<Adapter, Invalid> {
    let id = file.id.trim().to_owned();
    if !is_valid_id(&id) {
        return Err(invalid(
            "id",
            format!(
                "invalid id {id:?}: use lowercase letters, digits and '-', starting with a letter, \
                 at most {ADAPTER_ID_MAX} characters"
            ),
        ));
    }
    let name = file.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > ADAPTER_NAME_MAX {
        return Err(invalid(
            "name",
            format!("name must have 1 to {ADAPTER_NAME_MAX} characters"),
        ));
    }
    let command = file.command.trim().to_owned();
    if command.is_empty() {
        return Err(invalid("command", "command must not be empty"));
    }

    let detect = match file.detect {
        Some(d) if d.command.trim().is_empty() => {
            return Err(invalid("detect", "detect.command must not be empty"))
        }
        Some(d) => Some(DetectSpec {
            command: d.command.trim().to_owned(),
            args: d.args,
        }),
        None => None,
    };

    let state = StateRules {
        idle_regex: checked_regex("idle_regex", file.state.idle_regex)?,
        busy_regex: checked_regex("busy_regex", file.state.busy_regex)?,
        awaiting_regex: checked_regex("awaiting_regex", file.state.awaiting_regex)?,
        quiet_ms: file.state.quiet_ms.unwrap_or(DEFAULT_QUIET_MS),
    };
    if !QUIET_MS_RANGE.contains(&state.quiet_ms) {
        return Err(invalid(
            "quiet_ms",
            format!(
                "quiet_ms must be between {} and {}",
                QUIET_MS_RANGE.start(),
                QUIET_MS_RANGE.end()
            ),
        ));
    }

    let inject = InjectRules {
        mode: file.inject.mode.unwrap_or(InjectMode::Stdin),
        submit: file.inject.submit.unwrap_or_else(|| DEFAULT_SUBMIT.into()),
        prefix: file.inject.prefix.unwrap_or_else(|| DEFAULT_PREFIX.into()),
        max_chars: file.inject.max_chars.unwrap_or(DEFAULT_MAX_CHARS),
    };
    if inject.max_chars == 0 {
        return Err(invalid("max_chars", "max_chars must be greater than zero"));
    }

    let skills = match file.skills {
        Some(s) if s.dir.trim().is_empty() => {
            return Err(invalid("dir", "skills.dir must not be empty"))
        }
        Some(s) => Some(SkillsTarget {
            dir: s.dir.trim().to_owned(),
            format: s.format,
            settings_file: s.settings_file,
        }),
        None => None,
    };

    for key in file.env.keys() {
        // Mesma regra do agente (invariante I7 em `docs/04`): `AISENSE_*` carrega a
        // identidade no barramento e não pode vir de configuração.
        if key.to_ascii_uppercase().starts_with(RESERVED_ENV_PREFIX) {
            return Err(invalid(
                "env",
                format!("environment variable {key} is managed by AISENSE"),
            ));
        }
        if !is_valid_env_key(key) {
            return Err(invalid(
                "env",
                format!("invalid environment variable name {key:?}"),
            ));
        }
    }

    let caps = file.capabilities;
    Ok(Adapter {
        id,
        name,
        description: file.description.trim().to_owned(),
        icon: file.icon,
        command,
        args: file.args,
        detect,
        install_hint: file.install_hint,
        capabilities: Capabilities {
            mcp: caps.mcp,
            system_prompt_flag: caps.system_prompt_flag,
            hooks: caps.hooks,
            model_flag: caps.model_flag,
            cwd_is_project: caps.cwd_is_project,
            resume_flag: caps.resume_flag,
        },
        state,
        inject,
        skills,
        env: file.env,
        source,
    })
}

fn checked_regex(key: &'static str, value: Option<String>) -> Result<Option<String>, Invalid> {
    match value {
        Some(pattern) => match regex::Regex::new(&pattern) {
            Ok(_) => Ok(Some(pattern)),
            Err(err) => Err(invalid(key, format!("{key} is not a valid regex: {err}"))),
        },
        None => Ok(None),
    }
}

pub(crate) fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && id.len() <= ADAPTER_ID_MAX
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Converte um deslocamento em bytes para linha e coluna, ambas a partir de 1.
fn line_col(source: &str, offset: usize) -> (u32, u32) {
    let before = source.get(..offset).unwrap_or(source);
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |tail| tail.chars().count())
        + 1;
    (to_u32(line), to_u32(column))
}

/// Linha da primeira atribuição `key = ...` (ou tabela `[key]`). Melhor esforço: se o
/// campo faltou, não há linha para apontar e o aviso sai só com o caminho.
fn find_key_line(source: &str, key: &str) -> Option<u32> {
    source
        .lines()
        .position(|line| {
            let line = line.trim_start();
            let table = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']').or(Some(rest)))
                .is_some_and(|name| name.trim() == key);
            let assignment = line
                .strip_prefix(key)
                .is_some_and(|rest| rest.trim_start().starts_with(['=', '.']));
            table || assignment
        })
        .map(|index| to_u32(index + 1))
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    const MINIMAL: &str = "id = \"demo\"\nname = \"Demo\"\ncommand = \"demo\"\n";

    fn parse(src: &str) -> Result<Adapter, AdapterProblem> {
        parse_adapter(src, "demo.toml", AdapterSource::Builtin)
    }

    #[test]
    fn minimal_file_gets_defaults() {
        let a = parse(MINIMAL).unwrap();
        assert_eq!(a.id, "demo");
        assert_eq!(a.state.quiet_ms, DEFAULT_QUIET_MS);
        assert_eq!(a.inject.mode, InjectMode::Stdin);
        assert_eq!(a.inject.submit, "\r");
        assert_eq!(a.inject.max_chars, DEFAULT_MAX_CHARS);
        assert!(!a.capabilities.mcp);
        assert!(a.detect.is_none());
    }

    #[test]
    fn full_example_from_the_docs_parses() {
        let src = r#"
id          = "claude"
name        = "Claude Code"
description = "CLI oficial da Anthropic"
icon        = "sparkles"
command = "claude"
args    = []
detect       = { command = "claude", args = ["--version"] }
install_hint = "npm i -g @anthropic-ai/claude-code"

[capabilities]
mcp                = true
system_prompt_flag = "--append-system-prompt"
hooks              = true
model_flag         = "--model"
cwd_is_project     = true
resume_flag        = "--continue"

[state]
idle_regex     = '(?m)^\s*(?:│\s*)?[>❯]\s*$'
busy_regex     = '(?i)(thinking|working|esc to interrupt)'
awaiting_regex = '(?i)(do you want|\(y/n\)|\[y/N\]|permission)'
quiet_ms       = 400

[inject]
mode      = "stdin"
submit    = "\r"
prefix    = "[AISENSE] "
max_chars = 4000

[skills]
dir           = ".claude/skills"
format        = "claude-skill"
settings_file = ".claude/settings.json"

[env]
CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC = "1"
"#;
        let a = parse(src).unwrap();
        assert!(a.capabilities.mcp && a.capabilities.hooks);
        assert_eq!(a.detect.unwrap().args, vec!["--version"]);
        assert_eq!(a.skills.unwrap().dir, ".claude/skills");
        assert_eq!(a.env.len(), 1);
    }

    #[test]
    fn syntax_error_reports_line_and_column() {
        let src = "id = \"demo\"\nname = \"Demo\"\ncommand = \n";
        let problem = parse(src).unwrap_err();
        assert_eq!(problem.path, "demo.toml");
        assert_eq!(problem.line, Some(3));
        assert!(problem.column.is_some());
    }

    #[test]
    fn unknown_field_is_an_error_with_its_line() {
        let src = format!("{MINIMAL}\n[state]\nidle_regx = '>'\n");
        let problem = parse(&src).unwrap_err();
        assert_eq!(problem.line, Some(6), "{problem}");
        assert!(problem.message.contains("idle_regx"), "{problem}");
    }

    #[test]
    fn missing_required_field_is_reported() {
        let problem = parse("id = \"demo\"\nname = \"Demo\"\n").unwrap_err();
        assert!(problem.message.contains("command"), "{problem}");
    }

    #[test]
    fn invalid_regex_points_at_its_line() {
        let src = format!("{MINIMAL}[state]\nbusy_regex = '(unclosed'\n");
        let problem = parse(&src).unwrap_err();
        assert_eq!(problem.line, Some(5), "{problem}");
        assert!(problem.message.contains("busy_regex"));
    }

    #[test]
    fn invalid_ids_are_refused() {
        for id in ["", "Claude", "1abc", "has space", "under_score"] {
            let src = format!("id = {id:?}\nname = \"X\"\ncommand = \"x\"\n");
            let problem = parse(&src).unwrap_err();
            assert_eq!(problem.line, Some(1), "id {id:?}: {problem}");
        }
    }

    #[test]
    fn reserved_env_is_refused() {
        let src = format!("{MINIMAL}[env]\nAISENSE_TOKEN = \"x\"\n");
        let problem = parse(&src).unwrap_err();
        assert_eq!(problem.line, Some(4), "{problem}");
    }

    #[test]
    fn quiet_ms_out_of_range_is_refused() {
        let src = format!("{MINIMAL}[state]\nquiet_ms = 1\n");
        assert!(parse(&src).is_err());
    }

    #[test]
    fn unknown_inject_mode_is_refused() {
        let src = format!("{MINIMAL}[inject]\nmode = \"telepathy\"\n");
        assert_eq!(parse(&src).unwrap_err().line, Some(5));
    }

    #[test]
    fn problem_displays_like_a_compiler_error() {
        let p = AdapterProblem {
            path: "a.toml".into(),
            line: Some(3),
            column: Some(7),
            message: "bad".into(),
        };
        assert_eq!(p.to_string(), "a.toml:3:7: bad");
    }
}
