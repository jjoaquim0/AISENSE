//! Onde o AISENSE guarda seus dados (`docs/02`, "Persistência e layout em disco").
//!
//! Só calcula caminhos — não cria nada. Quem escreve é quem cria o diretório.

use std::path::{Path, PathBuf};

/// Variável que sobrescreve o diretório de dados (testes, instalações portáteis).
pub const HOME_ENV: &str = "AISENSE_HOME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir(PathBuf);

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self(root.into())
    }

    /// `$AISENSE_HOME`, senão `%APPDATA%\AISENSE` no Windows ou `~/.aisense` nos demais.
    /// `None` só quando o SO não informa nem a pasta do usuário.
    pub fn resolve() -> Option<Self> {
        Self::resolve_from(|key| std::env::var_os(key).filter(|v| !v.is_empty()))
    }

    fn resolve_from(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<Self> {
        if let Some(home) = var(HOME_ENV) {
            return Some(Self::new(home));
        }
        if cfg!(windows) {
            if let Some(appdata) = var("APPDATA") {
                return Some(Self::new(Path::new(&appdata).join("AISENSE")));
            }
        }
        var("HOME")
            .or_else(|| var("USERPROFILE"))
            .map(|home| Self::new(Path::new(&home).join(".aisense")))
    }

    pub fn root(&self) -> &Path {
        &self.0
    }

    pub fn database(&self) -> PathBuf {
        self.0.join("aisense.db")
    }

    pub fn adapters(&self) -> PathBuf {
        self.0.join("adapters")
    }

    pub fn logs(&self) -> PathBuf {
        self.0.join("logs")
    }

    /// Endereço do barramento (`docs/02`): socket Unix em `run/`, named pipe no Windows.
    pub fn socket(&self) -> String {
        if cfg!(windows) {
            r"\\.\pipe\aisense".to_owned()
        } else {
            self.0
                .join("run")
                .join("aisense.sock")
                .display()
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;

    use super::*;

    fn resolve(vars: &[(&str, &str)]) -> Option<DataDir> {
        let map: HashMap<String, OsString> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
            .collect();
        DataDir::resolve_from(|k| map.get(k).cloned())
    }

    #[test]
    fn explicit_home_wins() {
        let dir = resolve(&[(HOME_ENV, "/data/aisense"), ("HOME", "/home/u")]);
        assert_eq!(dir, Some(DataDir::new("/data/aisense")));
    }

    #[cfg(not(windows))]
    #[test]
    fn defaults_to_a_dot_folder_in_home() {
        let dir = resolve(&[("HOME", "/home/u")]);
        assert_eq!(
            dir.map(|d| d.database()),
            Some(PathBuf::from("/home/u/.aisense/aisense.db"))
        );
    }

    #[test]
    fn no_home_at_all_is_reported() {
        assert_eq!(resolve(&[]), None);
    }
}
