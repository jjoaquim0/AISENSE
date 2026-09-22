//! Comandos expostos ao front-end. Todo tipo que cruza esta fronteira vem do core
//! e é exportado para TypeScript com `ts-rs` — nunca escreva o tipo à mão do outro
//! lado (regra R5 de AGENTS.md).

use aisense_core::AppInfo;

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo::current()
}
