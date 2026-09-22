//! Binário `aisense`: a interface que os agentes usam de dentro do próprio terminal.
//!
//! Precisa iniciar em poucos milissegundos — é executado a cada mensagem enviada.
//! Toda operação real vai pelo socket do barramento; este binário nunca toca o banco
//! (ver `docs/02-arquitetura.md`).
#![forbid(unsafe_code)]

use std::process::ExitCode;

/// Códigos de saída contratados com as IAs. Ver `docs/07-barramento-comunicacao.md`.
/// Reservados para as fases seguintes: o contrato de exit code já está documentado.
#[allow(dead_code)]
mod exit {
    pub const OK: u8 = 0;
    pub const USAGE: u8 = 1;
    pub const TIMEOUT: u8 = 2;
    pub const UNAVAILABLE: u8 = 3;
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("aisense {}", aisense_core::VERSION);
            ExitCode::from(exit::OK)
        }
        Some("whoami") => match std::env::var("AISENSE_AGENT_HANDLE") {
            Ok(handle) => {
                println!("@{handle}");
                ExitCode::from(exit::OK)
            }
            Err(_) => {
                eprintln!(
                    "Este terminal não foi iniciado pelo AISENSE (AISENSE_AGENT_HANDLE ausente)."
                );
                ExitCode::from(exit::UNAVAILABLE)
            }
        },
        _ => {
            eprintln!(
                "aisense {} — ainda em construção (Fase 05).",
                aisense_core::VERSION
            );
            eprintln!("Comandos disponíveis: whoami, --version");
            ExitCode::from(exit::USAGE)
        }
    }
}
