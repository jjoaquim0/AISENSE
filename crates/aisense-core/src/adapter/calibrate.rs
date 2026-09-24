//! Gravar as regras de estado ajustadas no modo calibração (T9 → Runtimes; F08-05).
//!
//! Um adaptador do usuário é editado no próprio arquivo. Um embutido não pode ser
//! editado (vem no binário): vira uma cópia em `~/.aisense/adapters/<id>.toml`, que tem
//! precedência sobre ele (`AdapterCatalog`). Em ambos os casos o arquivo novo é validado
//! com o mesmo parser da carga **antes** de ir para o disco — o calibrador nunca deixa
//! um adaptador quebrado para trás — e a escrita é atômica.
//!
//! O TOML é reescrito a partir da tabela, então comentários do arquivo original se
//! perdem. Para um arquivo do usuário isso é um custo real; a UI avisa antes de gravar.

use std::path::{Path, PathBuf};

use super::catalog::BUILTIN_ADAPTERS;
use super::file::parse_adapter;
use super::model::{Adapter, AdapterSource, StateRules};

#[derive(Debug, thiserror::Error)]
pub enum CalibrateError {
    #[error("the built-in adapter {0:?} was not found")]
    UnknownBuiltin(String),
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("the adjusted adapter is invalid: {0}")]
    Invalid(String),
}

impl CalibrateError {
    pub fn to_command_error(&self) -> crate::CommandError {
        let (code, hint) = match self {
            Self::Invalid(_) => (
                "invalid_state_rules",
                "Corrija o regex marcado e tente de novo.",
            ),
            Self::Parse { .. } => (
                "adapter_unreadable",
                "Corrija o arquivo do adaptador à mão; o modo calibração não mexe nele assim.",
            ),
            _ => (
                "adapter_write_failed",
                "Confira as permissões da pasta de adaptadores.",
            ),
        };
        crate::CommandError::new(code, self.to_string(), Some(hint.to_owned()))
    }
}

/// Texto TOML de origem do adaptador e onde a versão calibrada vai morar.
fn source_and_target(
    adapter: &Adapter,
    user_dir: &Path,
) -> Result<(String, PathBuf, bool), CalibrateError> {
    match &adapter.source {
        AdapterSource::User { path } => {
            let path = PathBuf::from(path);
            let text = std::fs::read_to_string(&path).map_err(|source| CalibrateError::Read {
                path: path.clone(),
                source,
            })?;
            Ok((text, path, false))
        }
        AdapterSource::Builtin => {
            let text = BUILTIN_ADAPTERS
                .iter()
                .find(|(name, text)| {
                    parse_adapter(text, name, AdapterSource::Builtin)
                        .is_ok_and(|a| a.id == adapter.id)
                })
                .map(|(_, text)| (*text).to_owned())
                .ok_or_else(|| CalibrateError::UnknownBuiltin(adapter.id.clone()))?;
            Ok((text, user_dir.join(format!("{}.toml", adapter.id)), true))
        }
    }
}

/// Grava `rules` como as regras de estado de `adapter`. Devolve o arquivo escrito.
pub fn save_state_rules(
    adapter: &Adapter,
    rules: &StateRules,
    user_dir: &Path,
) -> Result<PathBuf, CalibrateError> {
    let (text, target, from_builtin) = source_and_target(adapter, user_dir)?;
    let mut table: toml::Table =
        text.parse()
            .map_err(|e: toml::de::Error| CalibrateError::Parse {
                path: target.clone(),
                message: e.message().to_owned(),
            })?;

    let mut state = toml::Table::new();
    let patterns = [
        ("idle_regex", &rules.idle_regex),
        ("busy_regex", &rules.busy_regex),
        ("awaiting_regex", &rules.awaiting_regex),
    ];
    for (key, value) in patterns {
        if let Some(value) = value.as_deref().filter(|v| !v.is_empty()) {
            state.insert(key.into(), toml::Value::String(value.to_owned()));
        }
    }
    state.insert(
        "quiet_ms".into(),
        toml::Value::Integer(rules.quiet_ms.into()),
    );
    table.insert("state".into(), toml::Value::Table(state));

    let body =
        toml::to_string_pretty(&table).map_err(|e| CalibrateError::Invalid(e.to_string()))?;
    let header = if from_builtin {
        format!(
            "# Cópia do adaptador embutido {:?} com as regras de estado ajustadas no modo\n\
             # calibração do AISENSE. Tem precedência sobre o embutido; apague para voltar a ele.\n\n",
            adapter.id
        )
    } else {
        String::new()
    };
    let output = format!("{header}{body}");
    let shown = target.display().to_string();
    parse_adapter(&output, &shown, adapter.source.clone())
        .map_err(|problem| CalibrateError::Invalid(problem.message))?;

    let write_err = |source| CalibrateError::Write {
        path: target.clone(),
        source,
    };
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(write_err)?;
    }
    let tmp = target.with_extension("toml.tmp");
    std::fs::write(&tmp, output).map_err(write_err)?;
    std::fs::rename(&tmp, &target).map_err(write_err)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::adapter::AdapterCatalog;

    fn rules(idle: &str) -> StateRules {
        StateRules {
            idle_regex: Some(idle.into()),
            busy_regex: None,
            awaiting_regex: Some(r"\(y/n\)".into()),
            quiet_ms: 250,
        }
    }

    #[test]
    fn a_builtin_becomes_a_user_copy_that_wins() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = AdapterCatalog::load(dir.path());
        let shell = catalog.get("shell").unwrap();
        let path = save_state_rules(shell, &rules(r"❯\s*$"), dir.path()).unwrap();
        assert_eq!(path, dir.path().join("shell.toml"));

        let reloaded = AdapterCatalog::load(dir.path());
        assert!(reloaded.problems().is_empty(), "{:?}", reloaded.problems());
        let shell = reloaded.get("shell").unwrap();
        assert_eq!(shell.state, rules(r"❯\s*$"));
        assert!(matches!(shell.source, AdapterSource::User { .. }));
        // O resto do adaptador veio junto.
        assert_eq!(shell.command, "$SHELL");
        assert!(!shell.inject.boot);
    }

    #[test]
    fn a_user_adapter_is_edited_in_place() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("mine.toml"),
            "id = \"mine\"\nname = \"Meu\"\ncommand = \"mine\"\n[state]\nidle_regex = 'x'\n",
        )
        .unwrap();
        let catalog = AdapterCatalog::load(dir.path());
        let mine = catalog.get("mine").unwrap();
        save_state_rules(mine, &rules("pronto>"), dir.path()).unwrap();
        let reloaded = AdapterCatalog::load(dir.path());
        assert_eq!(reloaded.get("mine").unwrap().state, rules("pronto>"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn an_invalid_regex_never_reaches_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = AdapterCatalog::load(dir.path());
        let shell = catalog.get("shell").unwrap();
        let err = save_state_rules(shell, &rules("(quebrado"), dir.path()).unwrap_err();
        assert!(matches!(err, CalibrateError::Invalid(_)), "{err}");
        assert!(!dir.path().join("shell.toml").exists());
    }
}
