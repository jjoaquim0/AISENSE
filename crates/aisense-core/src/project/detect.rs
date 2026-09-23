//! Sem `aisense.toml`, o app olha o diretório e **propõe** um — nunca cria sozinho
//! (`docs/17`, "Detecção automática"). A proposta vai para a UI revisar.

use std::fmt::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::config::{parse_project_config, ConfigProblem, ProjectConfig, FILE_NAME};

/// O que o app sabe sobre os comandos de um diretório.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum ProjectLookup {
    Found {
        path: String,
        config: ProjectConfig,
    },
    /// O arquivo existe mas não carregou. Nunca derruba o app.
    Invalid {
        problem: ConfigProblem,
    },
    /// Não há arquivo. `proposal` é o TOML sugerido, quando algo foi reconhecido.
    Missing {
        proposal: Option<Proposal>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Proposal {
    /// O arquivo que levou à proposta (`pnpm-lock.yaml`, `Cargo.toml`...).
    pub detected_from: String,
    pub toml: String,
}

pub fn load_project(dir: &Path) -> ProjectLookup {
    let path = dir.join(FILE_NAME);
    let shown = path.display().to_string();
    match std::fs::read_to_string(&path) {
        Ok(source) => match parse_project_config(&source, &shown) {
            Ok(config) => ProjectLookup::Found {
                path: shown,
                config,
            },
            Err(problem) => ProjectLookup::Invalid { problem },
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ProjectLookup::Missing {
            proposal: propose(dir),
        },
        Err(err) => ProjectLookup::Invalid {
            problem: ConfigProblem {
                path: shown,
                line: None,
                column: None,
                message: format!("could not read the file: {err}"),
            },
        },
    }
}

/// Um conjunto de comandos reconhecido, na ordem em que vai para o arquivo.
struct Detected {
    from: &'static str,
    commands: Vec<(&'static str, String)>,
    setup: Option<String>,
}

pub fn propose(dir: &Path) -> Option<Proposal> {
    let detected = node(dir)
        .or_else(|| rust(dir))
        .or_else(|| python(dir))
        .or_else(|| make(dir))?;
    Some(Proposal {
        detected_from: detected.from.to_owned(),
        toml: render(dir, &detected),
    })
}

fn node(dir: &Path) -> Option<Detected> {
    let (from, pm, install) = if dir.join("pnpm-lock.yaml").is_file() {
        ("pnpm-lock.yaml", "pnpm", "pnpm install")
    } else if dir.join("package-lock.json").is_file() {
        ("package-lock.json", "npm", "npm ci")
    } else if dir.join("yarn.lock").is_file() {
        ("yarn.lock", "yarn", "yarn install")
    } else {
        return None;
    };
    // Só propõe os scripts que existem: `pnpm lint` num projeto sem lint só confunde.
    let scripts = std::fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("scripts").cloned())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let run = |script: &str| match pm {
        "npm" if script == "test" => "npm test".to_owned(),
        "npm" => format!("npm run {script}"),
        _ => format!("{pm} {script}"),
    };
    let mut commands = vec![("install", install.to_owned())];
    for script in ["dev", "test", "lint", "build"] {
        if scripts.contains_key(script) {
            commands.push((script, run(script)));
        }
    }
    Some(Detected {
        from,
        commands,
        setup: Some(install.to_owned()),
    })
}

fn rust(dir: &Path) -> Option<Detected> {
    dir.join("Cargo.toml").is_file().then(|| Detected {
        from: "Cargo.toml",
        commands: vec![
            ("build", "cargo build".to_owned()),
            ("test", "cargo test".to_owned()),
            (
                "lint",
                "cargo clippy --all-targets -- -D warnings".to_owned(),
            ),
        ],
        setup: None,
    })
}

fn python(dir: &Path) -> Option<Detected> {
    (dir.join("pyproject.toml").is_file() && dir.join("uv.lock").is_file()).then(|| Detected {
        from: "uv.lock",
        commands: vec![
            ("install", "uv sync".to_owned()),
            ("test", "uv run pytest".to_owned()),
        ],
        setup: Some("uv sync".to_owned()),
    })
}

/// Os alvos do Makefile viram comandos com o mesmo nome (os que cabem na regra de nome).
fn make(dir: &Path) -> Option<Detected> {
    let source = std::fs::read_to_string(dir.join("Makefile")).ok()?;
    let mut commands: Vec<(&'static str, String)> = Vec::new();
    for known in [
        "install", "dev", "build", "test", "lint", "check", "fmt", "run", "clean",
    ] {
        let defines = source.lines().any(|line| {
            line.strip_prefix(known)
                .is_some_and(|rest| rest.starts_with(':') && !rest.starts_with(":="))
        });
        if defines {
            commands.push((known, format!("make {known}")));
        }
    }
    (!commands.is_empty()).then_some(Detected {
        from: "Makefile",
        commands,
        setup: None,
    })
}

fn render(dir: &Path, detected: &Detected) -> String {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Proposto pelo AISENSE a partir de {}.",
        detected.from
    );
    let _ = writeln!(
        out,
        "# Revise antes de aceitar: os agentes rodam exatamente isto.\n"
    );
    let _ = writeln!(out, "[project]\nname = {}\n", toml_string(&name));
    let _ = writeln!(out, "[commands]");
    let width = detected
        .commands
        .iter()
        .map(|(k, _)| k.len())
        .max()
        .unwrap_or(0);
    for (key, run) in &detected.commands {
        let _ = writeln!(out, "{key:<width$} = {}", toml_string(run));
    }
    let _ = writeln!(out, "\n[bench]\n# Arquivos ignorados pelo git que cada bancada precisa (ex.: \".env\").\ncopy = []");
    if let Some(setup) = &detected.setup {
        let _ = writeln!(out, "setup = {}", toml_string(setup));
    }
    let gates: Vec<&str> = ["lint", "test"]
        .into_iter()
        .filter(|g| detected.commands.iter().any(|(k, _)| k == g))
        .collect();
    if !gates.is_empty() {
        let list = gates
            .iter()
            .map(|g| format!("\"{g}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "\n[gates]\nreview = [{list}]");
    }
    out
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::path::PathBuf;

    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("aisense-proj-{}", ulid::Ulid::new()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn write(&self, name: &str, content: &str) {
            std::fs::write(self.0.join(name), content).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A proposta precisa ser um `aisense.toml` válido — é o que o usuário vai aceitar.
    fn proposal_of(dir: &TempDir) -> (Proposal, ProjectConfig) {
        let ProjectLookup::Missing {
            proposal: Some(proposal),
        } = load_project(&dir.0)
        else {
            panic!("esperava uma proposta");
        };
        let config = parse_project_config(&proposal.toml, "proposta").unwrap();
        (proposal, config)
    }

    #[test]
    fn pnpm_project_proposes_only_existing_scripts() {
        let dir = TempDir::new();
        dir.write("pnpm-lock.yaml", "");
        dir.write(
            "package.json",
            r#"{ "scripts": { "test": "vitest", "build": "vite build" } }"#,
        );
        let (proposal, config) = proposal_of(&dir);
        assert_eq!(proposal.detected_from, "pnpm-lock.yaml");
        let names: Vec<&str> = config.commands.keys().map(String::as_str).collect();
        assert_eq!(names, vec!["build", "install", "test"]);
        assert_eq!(config.commands["test"].run, "pnpm test");
        assert_eq!(config.bench.setup.as_deref(), Some("pnpm install"));
        assert_eq!(config.gates.review, vec!["test"]);
    }

    #[test]
    fn npm_uses_ci_and_run() {
        let dir = TempDir::new();
        dir.write("package-lock.json", "{}");
        dir.write(
            "package.json",
            r#"{ "scripts": { "test": "jest", "lint": "eslint ." } }"#,
        );
        let (_, config) = proposal_of(&dir);
        assert_eq!(config.commands["install"].run, "npm ci");
        assert_eq!(config.commands["test"].run, "npm test");
        assert_eq!(config.commands["lint"].run, "npm run lint");
    }

    #[test]
    fn cargo_and_makefile_are_recognized() {
        let rust = TempDir::new();
        rust.write("Cargo.toml", "[package]\nname = \"x\"\n");
        assert_eq!(proposal_of(&rust).1.commands["test"].run, "cargo test");

        let make = TempDir::new();
        make.write(
            "Makefile",
            "VAR := 1\ntest: build\n\tgo test ./...\nbuild:\n\tgo build\n",
        );
        let names: Vec<String> = proposal_of(&make).1.commands.into_keys().collect();
        assert_eq!(names, vec!["build", "test"]);
    }

    #[test]
    fn nothing_recognized_means_no_proposal() {
        let dir = TempDir::new();
        assert_eq!(
            load_project(&dir.0),
            ProjectLookup::Missing { proposal: None }
        );
    }

    #[test]
    fn a_proposal_is_never_written_to_disk() {
        let dir = TempDir::new();
        dir.write("Cargo.toml", "");
        let _ = load_project(&dir.0);
        assert!(!dir.0.join(FILE_NAME).exists());
    }

    #[test]
    fn an_invalid_file_is_reported_with_its_line_not_a_crash() {
        let dir = TempDir::new();
        dir.write(FILE_NAME, "[commands]\ntest = \n");
        let ProjectLookup::Invalid { problem } = load_project(&dir.0) else {
            panic!("esperava Invalid");
        };
        assert!(problem.path.ends_with(FILE_NAME));
        assert_eq!(problem.line, Some(2));
    }
}
