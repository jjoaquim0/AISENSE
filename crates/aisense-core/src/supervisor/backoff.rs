//! Espera entre reinícios (`docs/02`: "política de reinício ... com backoff").
//!
//! Um agente que cai no boot (flag errada, chave de API ausente) reiniciaria em laço
//! apertado, enchendo o log e a CPU. A espera dobra a cada queda seguida e volta ao
//! começo quando o agente fica de pé tempo suficiente para ser considerado estável.

use std::time::Duration;

pub const FIRST_DELAY: Duration = Duration::from_secs(1);
pub const MAX_DELAY: Duration = Duration::from_secs(60);
/// Rodou pelo menos isto antes de cair: a queda não é do boot, recomeça do início.
pub const STABLE_AFTER: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Backoff {
    /// Quedas seguidas sem ficar estável.
    failures: u32,
}

impl Backoff {
    /// Quanto esperar antes do próximo início, dado quanto a execução que acabou durou.
    pub fn next_delay(&mut self, ran_for: Duration) -> Duration {
        if ran_for >= STABLE_AFTER {
            self.failures = 0;
        }
        let delay = FIRST_DELAY
            .checked_mul(1u32.checked_shl(self.failures).unwrap_or(u32::MAX))
            .map_or(MAX_DELAY, |d| d.min(MAX_DELAY));
        self.failures = self.failures.saturating_add(1);
        delay
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUICK: Duration = Duration::from_millis(100);

    #[test]
    fn doubles_until_the_cap() {
        let mut backoff = Backoff::default();
        let delays: Vec<u64> = (0..9)
            .map(|_| backoff.next_delay(QUICK).as_secs())
            .collect();
        assert_eq!(delays, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
    }

    #[test]
    fn never_overflows() {
        let mut backoff = Backoff::default();
        for _ in 0..200 {
            assert!(backoff.next_delay(QUICK) <= MAX_DELAY);
        }
    }

    #[test]
    fn a_stable_run_starts_over() {
        let mut backoff = Backoff::default();
        for _ in 0..5 {
            backoff.next_delay(QUICK);
        }
        assert_eq!(backoff.next_delay(STABLE_AFTER), FIRST_DELAY);
        assert_eq!(backoff.failures(), 1);
    }
}
