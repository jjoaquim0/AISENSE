//! aisense-store
#![forbid(unsafe_code)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

mod db;
mod error;

pub use db::{Store, MIGRATOR};
pub use error::StoreError;
