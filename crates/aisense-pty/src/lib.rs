//! Gestão de PTY: processos de agente em terminais reais.
//!
//! Este crate não conhece equipes, mensagens nem skills — só processos, bytes e
//! janelas de terminal (`docs/02-arquitetura.md`).
#![forbid(unsafe_code)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

mod batch;
mod error;
mod log;
mod manager;
mod ring;
mod session;
mod wire;

pub use batch::{Batcher, DEFAULT_WINDOW};
pub use error::PtyError;
pub use log::{SessionLog, DEFAULT_MAX_LOG_BYTES};
pub use manager::{OutputSink, PtyManager};
pub use ring::{RingBuffer, DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES};
pub use session::{PtySession, PtySpawn, TerminalSize};
pub use wire::{PtyData, PtyExit, SpawnRequest};
