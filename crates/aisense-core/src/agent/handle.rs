//! Endereço do agente no barramento (`docs/12`: "Endereço").

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::validation::ValidationError;

/// Handles que o barramento usa para outra coisa: `@all` é a equipe inteira e
/// `@voce` é o humano (`docs/07`). Um agente com esses nomes seria inalcançável.
pub const RESERVED_HANDLES: [&str; 2] = ["all", "voce"];

pub const HANDLE_MIN_LEN: usize = 2;
pub const HANDLE_MAX_LEN: usize = 32;

/// Por que um handle foi recusado. Separado do erro para a UI poder explicar
/// exatamente o que corrigir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleProblem {
    TooShort,
    TooLong,
    MustStartWithLetter,
    InvalidCharacter(char),
}

impl fmt::Display for HandleProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "must have at least {HANDLE_MIN_LEN} characters"),
            Self::TooLong => write!(f, "must have at most {HANDLE_MAX_LEN} characters"),
            Self::MustStartWithLetter => f.write_str("must start with a lowercase letter"),
            Self::InvalidCharacter(c) => {
                write!(f, "character {c:?} is not allowed (use a-z, 0-9 and '-')")
            }
        }
    }
}

/// Handle validado: casa com `^[a-z][a-z0-9-]{1,31}$` e não é reservado (I1).
///
/// Só existe `Handle` válido — a deserialização também valida, então um valor
/// vindo do banco ou do IPC nunca entra no domínio sem passar pela regra.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TS)]
#[ts(
    export,
    export_to = "../../../apps/desktop/src/types/generated/",
    type = "string"
)]
pub struct Handle(String);

impl Handle {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let invalid = |problem| ValidationError::InvalidHandle {
            handle: raw.to_owned(),
            problem,
        };
        let len = raw.chars().count();
        if len < HANDLE_MIN_LEN {
            return Err(invalid(HandleProblem::TooShort));
        }
        if len > HANDLE_MAX_LEN {
            return Err(invalid(HandleProblem::TooLong));
        }
        let mut chars = raw.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_lowercase()) {
            return Err(invalid(HandleProblem::MustStartWithLetter));
        }
        if let Some(bad) = chars.find(|c| !matches!(c, 'a'..='z' | '0'..='9' | '-')) {
            return Err(invalid(HandleProblem::InvalidCharacter(bad)));
        }
        if RESERVED_HANDLES.contains(&raw) {
            return Err(ValidationError::ReservedHandle(raw.to_owned()));
        }
        Ok(Self(raw.to_owned()))
    }

    /// Sugere um handle a partir do nome de exibição: "Revisor Sênior" → `revisor-senior`.
    ///
    /// É só uma sugestão para o formulário (`docs/09`, T5); o resultado passa pela
    /// mesma validação e pode ser recusado (por exemplo, "All" → reservado).
    pub fn suggest(name: &str) -> Option<Self> {
        let mut out = String::new();
        for c in name.chars().flat_map(char::to_lowercase) {
            let c = fold_accent(c);
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                out.push(c);
            } else if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        }
        let trimmed = out.trim_start_matches(|c: char| !c.is_ascii_lowercase());
        let mut candidate: String = trimmed.chars().take(HANDLE_MAX_LEN).collect();
        while candidate.ends_with('-') {
            candidate.pop();
        }
        Self::parse(&candidate).ok()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Remove acentos do português (e vizinhos comuns) sem puxar uma crate de Unicode.
fn fold_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

impl fmt::Display for Handle {
    /// Exibido com `@`, como no barramento.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl Serialize for Handle {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Handle {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<String> for Handle {
    type Error = ValidationError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Handle> for String {
    fn from(value: Handle) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn problem(raw: &str) -> HandleProblem {
        match Handle::parse(raw) {
            Err(ValidationError::InvalidHandle { problem, .. }) => problem,
            other => panic!("expected InvalidHandle for {raw:?}, got {other:?}"),
        }
    }

    #[test]
    fn accepts_valid_handles() {
        for ok in ["ab", "backend", "front-end", "qa2", "a-", &"a".repeat(32)] {
            assert!(Handle::parse(ok).is_ok(), "{ok:?} should be valid");
        }
    }

    #[test]
    fn rejects_by_length() {
        assert_eq!(problem(""), HandleProblem::TooShort);
        assert_eq!(problem("a"), HandleProblem::TooShort);
        assert_eq!(problem(&"a".repeat(33)), HandleProblem::TooLong);
    }

    #[test]
    fn rejects_a_bad_first_character() {
        for bad in ["1abc", "-abc", "Abc", "@backend", "_x"] {
            assert_eq!(problem(bad), HandleProblem::MustStartWithLetter, "{bad:?}");
        }
    }

    #[test]
    fn rejects_characters_outside_the_alphabet() {
        assert_eq!(problem("back end"), HandleProblem::InvalidCharacter(' '));
        assert_eq!(problem("backEnd"), HandleProblem::InvalidCharacter('E'));
        assert_eq!(problem("back_end"), HandleProblem::InvalidCharacter('_'));
        assert_eq!(problem("revisão"), HandleProblem::InvalidCharacter('ã'));
    }

    #[test]
    fn rejects_reserved_handles() {
        for reserved in RESERVED_HANDLES {
            assert_eq!(
                Handle::parse(reserved),
                Err(ValidationError::ReservedHandle(reserved.to_owned()))
            );
        }
    }

    #[test]
    fn deserialization_validates_too() {
        assert!(serde_json::from_str::<Handle>("\"backend\"").is_ok());
        assert!(serde_json::from_str::<Handle>("\"all\"").is_err());
        assert!(serde_json::from_str::<Handle>("\"Bad Handle\"").is_err());
    }

    #[test]
    fn displays_with_at_sign() {
        let h = Handle::parse("backend").unwrap();
        assert_eq!(h.to_string(), "@backend");
        assert_eq!(serde_json::to_string(&h).unwrap(), "\"backend\"");
    }

    #[test]
    fn suggests_a_handle_from_the_display_name() {
        let s = |name: &str| Handle::suggest(name).map(|h| h.as_str().to_owned());
        assert_eq!(s("Backend"), Some("backend".into()));
        assert_eq!(s("Revisor Sênior"), Some("revisor-senior".into()));
        assert_eq!(s("  Coordenação / QA  "), Some("coordenacao-qa".into()));
        assert_eq!(s("2º Dev"), Some("dev".into()));
        assert_eq!(s(&"Muito ".repeat(10)).map(|h| h.len() <= 32), Some(true));
        assert_eq!(s("All"), None, "reserved handles are never suggested");
        assert_eq!(s("!!!"), None);
    }
}
