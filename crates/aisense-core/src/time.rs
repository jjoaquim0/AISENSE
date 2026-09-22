//! Relógio do domínio.
//!
//! Todo timestamp é epoch em **milissegundos UTC** (`docs/04`). O domínio recebe o
//! instante como parâmetro em vez de ler o relógio sozinho: assim os testes são
//! determinísticos e ninguém guarda hora local por acidente.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milissegundos desde a época Unix, em UTC.
pub type Millis = i64;

/// Instante atual. Um relógio anterior a 1970 vira `0` em vez de pânico.
pub fn now_ms() -> Millis {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
