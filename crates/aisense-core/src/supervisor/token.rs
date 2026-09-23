//! `AISENSE_TOKEN`: credencial efêmera de uma execução de agente (`docs/11`).
//!
//! Vale só enquanto a sessão estiver viva. Quem o valida é o barramento (Fase 05), que
//! grava em `agent_tokens`; aqui só nasce, com entropia do sistema operacional.

use std::fmt::Write;

const TOKEN_BYTES: usize = 32;

#[derive(Debug, thiserror::Error)]
#[error("the operating system did not provide random bytes: {0}")]
pub struct TokenError(String);

/// 32 bytes aleatórios do SO, em hexadecimal (64 caracteres).
pub fn generate_token() -> Result<String, TokenError> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|e| TokenError(e.to_string()))?;
    Ok(bytes
        .iter()
        .fold(String::with_capacity(TOKEN_BYTES * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn tokens_are_long_hex_and_distinct() {
        let a = generate_token().unwrap();
        let b = generate_token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
