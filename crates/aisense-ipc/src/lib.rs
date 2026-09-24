//! aisense-ipc: o barramento no socket local (`docs/07`, ADR 0004).
//!
//! NDJSON sobre Unix domain socket / named pipe, `hello` com `AISENSE_TOKEN` obrigatório,
//! frame de até 1 MiB. A CLI `aisense` e o `aisense-mcp` são clientes; o app é o servidor.
#![forbid(unsafe_code)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

pub mod cli;
mod client;
mod frame;
mod handler;
mod protocol;
pub mod render;
mod transport;

pub use client::{Client, ClientError};
pub use frame::{read_frame, write_frame, FrameError, MAX_FRAME};
pub use handler::{bus_error, BusHandler, WAIT_DEFAULT};
pub use protocol::{codes, NotesOp, Request, Response, PROTOCOL_VERSION};
pub use transport::{connect, serve, serve_connection, BindError, Handler, Io};
