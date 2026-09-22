//! Aplicativo Tauri do AISENSE.
//!
//! Esta camada é **fina de propósito**: ela traduz UI ⇄ core e não contém regra de
//! negócio. Ver `docs/02-arquitetura.md` e a regra R6 de `AGENTS.md`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod commands;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("AISENSE_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!(version = aisense_core::VERSION, "AISENSE iniciando");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a janela do AISENSE");
}
