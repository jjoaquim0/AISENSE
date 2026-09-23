//! `aisense.toml`: como o projeto se opera (`docs/17`).

use std::collections::BTreeMap;
use std::path::Component;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::toml_pos::{find_key_line, line_col};

pub const FILE_NAME: &str = "aisense.toml";
pub const DEFAULT_TIMEOUT_S: u32 = 600;
pub const MAX_TIMEOUT_S: u32 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct ProjectConfig {
    pub name: Option<String>,
    /// Em ordem alfabética; é o que `aisense run <nome>` aceita — e só isso.
    pub commands: BTreeMap<String, ProjectCommand>,
    pub bench: BenchConfig,
    pub gates: GatesConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct ProjectCommand {
    pub run: String,
    pub timeout_s: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct BenchConfig {
    /// Arquivos ignorados pelo git que cada bancada precisa (`docs/16`), relativos à raiz.
    pub copy: Vec<String>,
    /// Rodado uma vez ao criar a bancada.
    pub setup: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct GatesConfig {
    /// Comandos que precisam passar antes de um cartão sair de "revisão" (`docs/13`).
    pub review: Vec<String>,
}

/// Arquivo que não carregou: caminho e, quando dá para saber, linha e coluna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct ConfigProblem {
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
}

impl std::fmt::Display for ConfigProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
            if let Some(column) = self.column {
                write!(f, ":{column}")?;
            }
        }
        write!(f, ": {}", self.message)
    }
}

// ─────────────────────────── formato em disco ───────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct File {
    project: ProjectSection,
    commands: BTreeMap<String, CommandEntry>,
    bench: BenchSection,
    gates: GatesSection,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct ProjectSection {
    name: Option<String>,
}

/// `test = "pnpm test"` ou `test = { run = "pnpm test", timeout_s = 1800 }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CommandEntry {
    Short(String),
    Full(FullCommand),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FullCommand {
    run: String,
    timeout_s: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct BenchSection {
    copy: Vec<String>,
    setup: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct GatesSection {
    review: Vec<String>,
}

/// Lê e valida. `path` só compõe a mensagem de erro.
pub fn parse_project_config(source: &str, path: &str) -> Result<ProjectConfig, ConfigProblem> {
    let file: File = toml::from_str(source).map_err(|err| {
        let (line, column) = err.span().map(|s| line_col(source, s.start)).unzip();
        ConfigProblem {
            path: path.to_owned(),
            line,
            column,
            message: err.message().trim().to_owned(),
        }
    })?;
    validate(file).map_err(|(key, message)| ConfigProblem {
        path: path.to_owned(),
        line: find_key_line(source, &key),
        column: None,
        message,
    })
}

/// `(campo para achar a linha, mensagem)`.
type Invalid = (String, String);

fn validate(file: File) -> Result<ProjectConfig, Invalid> {
    let mut commands = BTreeMap::new();
    for (name, entry) in file.commands {
        if !is_command_name(&name) {
            return Err((
                name.clone(),
                format!("invalid command name {name:?}: use lowercase letters, digits, '-' or '_'"),
            ));
        }
        let (run, timeout_s) = match entry {
            CommandEntry::Short(run) => (run, None),
            CommandEntry::Full(full) => (full.run, full.timeout_s),
        };
        let run = run.trim().to_owned();
        if run.is_empty() {
            return Err((name, "the command line must not be empty".to_owned()));
        }
        let timeout_s = timeout_s.unwrap_or(DEFAULT_TIMEOUT_S);
        if !(1..=MAX_TIMEOUT_S).contains(&timeout_s) {
            return Err((
                name,
                format!("timeout_s must be between 1 and {MAX_TIMEOUT_S}"),
            ));
        }
        commands.insert(name, ProjectCommand { run, timeout_s });
    }

    for gate in &file.gates.review {
        if !commands.contains_key(gate) {
            return Err((
                "review".to_owned(),
                format!("gate {gate:?} is not defined in [commands]"),
            ));
        }
    }

    for copied in &file.bench.copy {
        // Copiar para a bancada algo de fora do repositório (`../.ssh`, `C:\...`)
        // levaria segredos para onde o agente mexe à vontade.
        if !is_inside_the_repo(copied) {
            return Err((
                "copy".to_owned(),
                format!("{copied:?} must be a path relative to the repository, without '..'"),
            ));
        }
    }

    let setup = file
        .bench
        .setup
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    Ok(ProjectConfig {
        name: file
            .project
            .name
            .map(|n| n.trim().to_owned())
            .filter(|n| !n.is_empty()),
        commands,
        bench: BenchConfig {
            copy: file.bench.copy,
            setup,
        },
        gates: GatesConfig {
            review: file.gates.review,
        },
    })
}

fn is_command_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && name.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn is_inside_the_repo(path: &str) -> bool {
    let path = std::path::Path::new(path);
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn parse(src: &str) -> Result<ProjectConfig, ConfigProblem> {
        parse_project_config(src, "aisense.toml")
    }

    #[test]
    fn the_example_from_the_docs_parses() {
        let src = r#"
[project]
name = "minha-api"

[commands]
install = "pnpm install"
test    = { run = "pnpm test", timeout_s = 1800 }
migrate = "pnpm prisma migrate dev"

[bench]
copy = [".env", ".env.local"]
setup = "pnpm install"

[gates]
review = ["test"]
"#;
        let config = parse(src).unwrap();
        assert_eq!(config.name.as_deref(), Some("minha-api"));
        assert_eq!(config.commands["install"].timeout_s, DEFAULT_TIMEOUT_S);
        assert_eq!(config.commands["test"].timeout_s, 1800);
        assert_eq!(config.bench.copy, vec![".env", ".env.local"]);
        assert_eq!(config.gates.review, vec!["test"]);
    }

    #[test]
    fn an_empty_file_is_a_valid_empty_config() {
        let config = parse("").unwrap();
        assert!(config.commands.is_empty());
    }

    #[test]
    fn syntax_error_reports_path_and_line() {
        let problem = parse("[commands]\ntest = \n").unwrap_err();
        assert_eq!(problem.path, "aisense.toml");
        assert_eq!(problem.line, Some(2));
        assert!(problem.to_string().starts_with("aisense.toml:2"));
    }

    #[test]
    fn unknown_section_is_an_error() {
        let problem = parse("[comands]\ntest = \"x\"\n").unwrap_err();
        assert_eq!(problem.line, Some(1), "{problem}");
    }

    #[test]
    fn a_gate_must_name_an_existing_command() {
        let problem =
            parse("[commands]\ntest = \"t\"\n\n[gates]\nreview = [\"lint\"]\n").unwrap_err();
        assert_eq!(problem.line, Some(5), "{problem}");
        assert!(problem.message.contains("lint"));
    }

    #[test]
    fn bench_copy_cannot_leave_the_repository() {
        for bad in ["../.ssh/id_rsa", "/etc/passwd", ""] {
            let src = format!("[bench]\ncopy = [{bad:?}]\n");
            assert!(parse(&src).is_err(), "{bad:?} deveria ser recusado");
        }
        assert!(parse("[bench]\ncopy = [\"config/.env\"]\n").is_ok());
    }

    #[test]
    fn bad_command_names_and_empty_lines_are_refused() {
        assert!(parse("[commands]\nTest = \"x\"\n").is_err());
        let problem = parse("[commands]\ntest = \"  \"\n").unwrap_err();
        assert_eq!(problem.line, Some(2));
    }

    #[test]
    fn timeout_out_of_range_is_refused() {
        assert!(parse("[commands]\ntest = { run = \"t\", timeout_s = 0 }\n").is_err());
    }
}
