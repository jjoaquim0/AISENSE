//! IDs tipados. Nunca use `String` solta para identificar uma entidade: trocar
//! um `TeamId` por um `AgentId` numa chamada vira bug silencioso.

use std::fmt;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Gerador monotônico compartilhado.
///
/// `Ulid::new()` sozinho **não** é ordenável dentro do mesmo milissegundo: o sufixo é
/// aleatório. Como a linha do tempo e a caixa de entrada ordenam mensagens por ID, duas
/// mensagens criadas no mesmo milissegundo poderiam aparecer fora de ordem. O gerador
/// monotônico incrementa o sufixo quando o timestamp se repete, garantindo a ordem.
static GENERATOR: Mutex<Option<ulid::Generator>> = Mutex::new(None);

fn next_ulid() -> ulid::Ulid {
    let mut guard = match GENERATOR.lock() {
        Ok(guard) => guard,
        // Um lock envenenado significa pânico em outra thread; seguir com ID aleatório é
        // melhor que derrubar o processo por causa de um identificador.
        Err(poisoned) => poisoned.into_inner(),
    };
    let generator = guard.get_or_insert_with(ulid::Generator::new);
    // `generate` só falha se estourarem 2^80 IDs no mesmo milissegundo; nesse caso o ULID
    // aleatório é um fallback aceitável.
    generator.generate().unwrap_or_else(|_| ulid::Ulid::new())
}

macro_rules! typed_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        #[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
        pub struct $name(String);

        impl $name {
            /// Gera um novo ID ordenável por tempo (ULID com prefixo).
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, next_ulid()))
            }

            /// Constrói a partir de um valor já existente (banco, IPC, ambiente).
            pub fn from_raw(raw: impl Into<String>) -> Self {
                Self(raw.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Prefixo esperado para este tipo de ID.
            pub const PREFIX: &'static str = $prefix;
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

typed_id!(TeamId, "tem");
typed_id!(AgentId, "agt");
typed_id!(SkillId, "skl");
typed_id!(MessageId, "msg");
typed_id!(ChannelId, "chn");
typed_id!(SessionId, "ses");
typed_id!(BoardId, "brd");
typed_id!(CardId, "tsk");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_carry_their_prefix() {
        assert!(TeamId::new().as_str().starts_with("tem_"));
        assert!(CardId::new().as_str().starts_with("tsk_"));
    }

    #[test]
    fn ids_sort_by_creation_order_even_within_the_same_millisecond() {
        // 1.000 IDs seguidos caem quase todos no mesmo milissegundo: é exatamente o caso
        // que o gerador monotônico precisa cobrir.
        let ids: Vec<String> = (0..1_000)
            .map(|_| MessageId::new().as_str().to_owned())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(
            ids, sorted,
            "IDs devem sair em ordem crescente na ordem de criação"
        );
    }

    #[test]
    fn ids_are_unique_under_concurrency() {
        use std::collections::HashSet;
        let handles: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (0..500).map(|_| AgentId::new()).collect::<Vec<_>>()))
            .collect();
        let all: Vec<AgentId> = handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .flatten()
            .collect();
        let unique: HashSet<&AgentId> = all.iter().collect();
        assert_eq!(all.len(), 4_000);
        assert_eq!(
            unique.len(),
            all.len(),
            "nenhum ID pode se repetir entre threads"
        );
    }

    #[test]
    fn ids_are_unique() {
        let a = AgentId::new();
        let b = AgentId::new();
        assert_ne!(a, b);
    }
}
