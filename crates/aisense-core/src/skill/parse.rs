//! `SKILL.md`: frontmatter YAML entre `---` e o corpo em Markdown (`docs/06`, "Formato").
//!
//! O YAML é lido com `yaml-rust2` e mapeado à mão, não por serde: o erro precisa dizer a
//! **linha do arquivo** — de sintaxe (o parser dá a posição) ou de campo (achamos a
//! linha da chave). Campos desconhecidos são ignorados de propósito: skills do Claude
//! Code trazem `allowed-tools`, `license`, `metadata`... e têm de funcionar sem conversão.

use std::collections::BTreeMap;

use yaml_rust2::{Yaml, YamlLoader};

use super::model::{Skill, SkillInject, SkillProblem, SkillSource};
use crate::agent::RESERVED_ENV_PREFIX;

pub const SKILL_NAME_MAX: usize = 64;
/// O mesmo teto do Claude Code: uma frase, não um manual.
pub const SKILL_DESCRIPTION_MAX: usize = 1024;
/// Uma skill vai inteira para o `BOOT.md` (limite de 12.000 caracteres, `docs/06`);
/// um arquivo de 256 KB já é engano, não skill.
pub const SKILL_FILE_MAX_BYTES: usize = 256 * 1024;
const DEFAULT_VERSION: &str = "1.0.0";
const DEFAULT_PRIORITY: u8 = 50;

/// Um erro de validação: a chave em que ele está e a mensagem.
struct Invalid {
    key: Option<&'static str>,
    message: String,
}

fn invalid(key: &'static str, message: impl Into<String>) -> Invalid {
    Invalid {
        key: Some(key),
        message: message.into(),
    }
}

/// Lê e valida um `SKILL.md`. `path` só compõe a mensagem de erro.
pub fn parse_skill(source: &str, path: &str, origin: SkillSource) -> Result<Skill, SkillProblem> {
    let problem = |line: Option<u32>, message: String| SkillProblem {
        path: path.to_owned(),
        line,
        message,
    };
    if source.len() > SKILL_FILE_MAX_BYTES {
        return Err(problem(
            None,
            format!("SKILL.md is larger than {} KB", SKILL_FILE_MAX_BYTES / 1024),
        ));
    }
    let Some(front) = split_frontmatter(source) else {
        return Err(problem(
            Some(1),
            "SKILL.md must start with a YAML frontmatter between `---` lines".into(),
        ));
    };

    let docs = YamlLoader::load_from_str(front.yaml).map_err(|err| {
        // O parser conta a partir da primeira linha do YAML; o arquivo, do `---`.
        let line = front.first_line + to_u32(err.marker().line()).saturating_sub(1);
        problem(Some(line), format!("invalid YAML: {}", err.info()))
    })?;
    let root = docs.into_iter().next().unwrap_or(Yaml::Null);

    validate(&root, front.body, origin).map_err(|bad| {
        let line = bad
            .key
            .and_then(|key| find_yaml_key_line(front.yaml, key))
            .map(|l| front.first_line + l - 1);
        problem(line, bad.message)
    })
}

struct Frontmatter<'a> {
    yaml: &'a str,
    body: &'a str,
    /// Linha do arquivo onde o YAML começa (a seguinte ao `---`).
    first_line: u32,
}

/// Separa `---\n<yaml>\n---\n<corpo>`. Aceita BOM e `\r\n` (arquivos do Windows).
fn split_frontmatter(source: &str) -> Option<Frontmatter<'_>> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let rest = source
        .strip_prefix("---\r\n")
        .or_else(|| source.strip_prefix("---\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            let yaml = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return Some(Frontmatter {
                yaml,
                body,
                first_line: 2,
            });
        }
        offset += line.len();
    }
    None
}

fn validate(root: &Yaml, body: &str, source: SkillSource) -> Result<Skill, Invalid> {
    let map = match root {
        Yaml::Hash(map) => map,
        Yaml::Null => {
            return Err(Invalid {
                key: None,
                message: "the frontmatter is empty; `name` and `description` are required".into(),
            })
        }
        _ => {
            return Err(Invalid {
                key: None,
                message: "the frontmatter must be a list of `key: value` fields".into(),
            })
        }
    };
    let get = |key: &str| map.get(&Yaml::String(key.to_owned()));

    let name = required_text(get("name"), "name")?;
    validate_name(&name)?;

    let description = required_text(get("description"), "description")?;
    if description.chars().count() > SKILL_DESCRIPTION_MAX {
        return Err(invalid(
            "description",
            format!("description must be at most {SKILL_DESCRIPTION_MAX} characters"),
        ));
    }

    let version = match get("version") {
        None | Some(Yaml::Null) => DEFAULT_VERSION.to_owned(),
        Some(Yaml::String(v)) if is_semver(v) => v.clone(),
        // `version: 1.2` vira número em YAML: o aviso ensina a corrigir.
        Some(Yaml::Real(v)) => {
            return Err(invalid(
                "version",
                format!("version must be a SemVer string; write it quoted, like \"{v}.0\""),
            ))
        }
        Some(_) => {
            return Err(invalid(
                "version",
                "version must be a SemVer string like \"1.2.0\"",
            ))
        }
    };

    let targets = match get("targets") {
        None | Some(Yaml::Null) => Vec::new(),
        Some(Yaml::Array(items)) => items
            .iter()
            .map(|item| match item {
                Yaml::String(id) if is_adapter_id(id) => Ok(id.clone()),
                _ => Err(invalid(
                    "targets",
                    "targets must list runtime ids like [claude, codex]",
                )),
            })
            .collect::<Result<_, _>>()?,
        Some(_) => {
            return Err(invalid(
                "targets",
                "targets must be a list, like [claude, codex]",
            ))
        }
    };

    let inject = match get("inject") {
        None | Some(Yaml::Null) => SkillInject::default(),
        Some(Yaml::String(mode)) => match mode.as_str() {
            "bootstrap" => SkillInject::Bootstrap,
            "reference" => SkillInject::Reference,
            "mcp" => SkillInject::Mcp,
            other => {
                return Err(invalid(
                    "inject",
                    format!("inject must be bootstrap, reference or mcp, not {other:?}"),
                ))
            }
        },
        Some(_) => {
            return Err(invalid(
                "inject",
                "inject must be bootstrap, reference or mcp",
            ))
        }
    };

    let priority = match get("priority") {
        None | Some(Yaml::Null) => DEFAULT_PRIORITY,
        Some(Yaml::Integer(p)) => u8::try_from(*p)
            .ok()
            .filter(|p| *p <= 100)
            .ok_or_else(|| invalid("priority", "priority must be between 0 and 100"))?,
        Some(_) => {
            return Err(invalid(
                "priority",
                "priority must be a whole number from 0 to 100",
            ))
        }
    };

    let env = match get("env") {
        None | Some(Yaml::Null) => BTreeMap::new(),
        Some(Yaml::Hash(vars)) => {
            let mut env = BTreeMap::new();
            for (key, value) in vars {
                let Yaml::String(key) = key else {
                    return Err(invalid("env", "env keys must be variable names"));
                };
                validate_env_key(key)?;
                let value = scalar_text(value).ok_or_else(|| {
                    invalid(
                        "env",
                        format!("env {key} must be a text, number or boolean"),
                    )
                })?;
                env.insert(key.clone(), value);
            }
            env
        }
        Some(_) => return Err(invalid("env", "env must be a map of VARIABLE: value")),
    };

    let body = body.trim().to_owned();
    if body.is_empty() {
        return Err(Invalid {
            key: None,
            message: "the skill has no instructions after the frontmatter".into(),
        });
    }

    Ok(Skill {
        name,
        description,
        version,
        targets,
        inject,
        priority,
        env,
        body,
        source,
    })
}

fn required_text(value: Option<&Yaml>, key: &'static str) -> Result<String, Invalid> {
    match value {
        None | Some(Yaml::Null) => Err(invalid(key, format!("{key} is required"))),
        Some(Yaml::String(text)) if !text.trim().is_empty() => Ok(text.trim().to_owned()),
        Some(Yaml::String(_)) => Err(invalid(key, format!("{key} must not be empty"))),
        Some(_) => Err(invalid(key, format!("{key} must be text"))),
    }
}

fn validate_name(name: &str) -> Result<(), Invalid> {
    let mut chars = name.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid {
        return Err(invalid(
            "name",
            format!(
                "name {name:?} must use lowercase letters, digits and hyphens, starting with a letter"
            ),
        ));
    }
    if name.len() > SKILL_NAME_MAX {
        return Err(invalid(
            "name",
            format!("name must be at most {SKILL_NAME_MAX} characters"),
        ));
    }
    Ok(())
}

/// `MAJOR.MINOR.PATCH`, com pré-lançamento/metadados opcionais (`1.0.0-beta+2`).
fn is_semver(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<_> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Mesma forma dos ids de adaptador (`claude`, `open-code`).
fn is_adapter_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn validate_env_key(key: &str) -> Result<(), Invalid> {
    let mut chars = key.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(invalid(
            "env",
            format!("invalid environment variable name {key:?}"),
        ));
    }
    // Identidade do agente no barramento (invariante I7, `docs/04`).
    if key.to_ascii_uppercase().starts_with(RESERVED_ENV_PREFIX) {
        return Err(invalid(
            "env",
            format!("environment variable {key} is managed by AISENSE and cannot be overridden"),
        ));
    }
    Ok(())
}

/// `REVIEW_STRICT: 1` e `DEBUG: true` são comuns; viram texto como o shell os veria.
fn scalar_text(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Linha (a partir de 1, dentro do YAML) da chave de primeiro nível `key:`.
fn find_yaml_key_line(yaml: &str, key: &str) -> Option<u32> {
    yaml.lines()
        .position(|line| {
            line.strip_prefix(key)
                .is_some_and(|rest| rest.trim_start().starts_with(':'))
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

    fn parse(source: &str) -> Result<Skill, SkillProblem> {
        parse_skill(source, "revisor/SKILL.md", SkillSource::Builtin)
    }

    const EXAMPLE: &str = "---
name: revisor-rigoroso
description: Revisa diffs procurando bugs de correção, não estilo. Use antes de abrir PR.
version: 1.2.0
targets: [claude, codex, opencode]
inject: bootstrap
priority: 40
env:
  REVIEW_STRICT: \"1\"
---

# Revisor rigoroso

Você revisa código procurando **defeitos de correção**.
";

    #[test]
    fn le_o_exemplo_do_doc_06() {
        let skill = parse(EXAMPLE).unwrap();
        assert_eq!(skill.name, "revisor-rigoroso");
        assert_eq!(skill.version, "1.2.0");
        assert_eq!(skill.targets, ["claude", "codex", "opencode"]);
        assert_eq!(skill.inject, SkillInject::Bootstrap);
        assert_eq!(skill.priority, 40);
        assert_eq!(
            skill.env.get("REVIEW_STRICT").map(String::as_str),
            Some("1")
        );
        assert!(skill.body.starts_with("# Revisor rigoroso"));
        assert!(skill.supports("codex"));
        assert!(!skill.supports("shell"));
    }

    #[test]
    fn padroes_quando_os_campos_opcionais_faltam() {
        let skill = parse("---\nname: x\ndescription: Faz x.\n---\ncorpo\n").unwrap();
        assert_eq!(skill.version, "1.0.0");
        assert!(skill.targets.is_empty());
        assert!(skill.supports("qualquer"));
        assert_eq!(skill.priority, 50);
        assert_eq!(skill.inject, SkillInject::Bootstrap);
    }

    #[test]
    fn skill_do_claude_code_carrega_sem_conversao() {
        // Campos que o Claude Code usa e o AISENSE não conhece: ignorados, não erro.
        let src = "---\nname: pdf\ndescription: Lida com PDFs.\nlicense: MIT\nallowed-tools: Read, Bash\nmetadata:\n  autor: x\n---\n# PDF\n";
        assert_eq!(parse(src).unwrap().name, "pdf");
    }

    #[test]
    fn sem_description_e_recusada_com_a_linha_do_nome() {
        let problem = parse("---\nname: x\n---\ncorpo\n").unwrap_err();
        assert!(
            problem.message.contains("description is required"),
            "{problem}"
        );
        let problem = parse("---\nname: x\ndescription: \"  \"\n---\ncorpo\n").unwrap_err();
        assert_eq!(problem.line, Some(3), "{problem}");
    }

    #[test]
    fn yaml_quebrado_aponta_a_linha_do_arquivo() {
        let problem =
            parse("---\nname: x\ndescription: [aberto\npriority: 3\n---\ncorpo\n").unwrap_err();
        assert!(problem.message.starts_with("invalid YAML"), "{problem}");
        let line = problem.line.unwrap();
        assert!((3..=5).contains(&line), "linha {line}: {problem}");
        assert!(problem.to_string().starts_with("revisor/SKILL.md:"));
    }

    #[test]
    fn campo_invalido_aponta_a_propria_linha() {
        let cases = [
            ("name: Revisor", 2),
            ("priority: 200", 4),
            ("inject: sempre", 4),
            ("version: 1.2", 4),
            ("targets: claude", 4),
            ("env:\n  AISENSE_TOKEN: x", 4),
        ];
        for (field, line) in cases {
            let src = if field.starts_with("name") {
                format!("---\n{field}\ndescription: d\n---\ncorpo\n")
            } else {
                format!("---\nname: x\ndescription: d\n{field}\n---\ncorpo\n")
            };
            let problem = parse(&src).unwrap_err();
            assert_eq!(problem.line, Some(line), "{field}: {problem}");
        }
    }

    #[test]
    fn versao_numerica_ensina_a_por_aspas() {
        let problem = parse("---\nname: x\ndescription: d\nversion: 1.2\n---\nc\n").unwrap_err();
        assert!(problem.message.contains("\"1.2.0\""), "{problem}");
    }

    #[test]
    fn sem_frontmatter_ou_sem_corpo_e_recusada() {
        assert_eq!(parse("# só markdown\n").unwrap_err().line, Some(1));
        assert!(
            parse("---\nname: x\ndescription: d\n").is_err(),
            "sem fechar o ---"
        );
        let problem = parse("---\nname: x\ndescription: d\n---\n\n").unwrap_err();
        assert!(problem.message.contains("no instructions"), "{problem}");
        assert!(parse("---\n---\ncorpo")
            .unwrap_err()
            .message
            .contains("empty"));
    }

    #[test]
    fn aceita_bom_e_fim_de_linha_do_windows() {
        let src = "\u{feff}---\r\nname: x\r\ndescription: d\r\n---\r\ncorpo\r\n";
        assert_eq!(parse(src).unwrap().body, "corpo");
    }

    #[test]
    fn env_aceita_numero_e_booleano_como_texto() {
        let src = "---\nname: x\ndescription: d\nenv:\n  N: 1\n  B: true\n---\nc\n";
        let env = parse(src).unwrap().env;
        assert_eq!((env["N"].as_str(), env["B"].as_str()), ("1", "true"));
    }

    #[test]
    fn nunca_entra_em_panico_com_lixo() {
        for src in [
            "---\n: :\n---\nx",
            "---\n- a\n- b\n---\nx",
            "---\nname: [1, 2]\ndescription: {a: b}\n---\nx",
            "---\n\t\u{0}\n---\nx",
            "---\nname: x\ndescription: d\npriority: -1\n---\nx",
            "---\nname: x\ndescription: d\npriority: 99999999999999999999\n---\nx",
        ] {
            assert!(parse(src).is_err(), "{src:?}");
        }
        let huge = format!(
            "---\nname: x\ndescription: d\n---\n{}",
            "a".repeat(SKILL_FILE_MAX_BYTES)
        );
        assert!(parse(&huge).unwrap_err().message.contains("larger"));
    }
}
