//! Binário `aisense-mcp`: servidor MCP stdio que expõe o barramento e o quadro
//! como ferramentas nativas para runtimes compatíveis.
//!
//! Implementação real na Fase 05 (F05-09). Ver `docs/07-barramento-comunicacao.md`.
#![forbid(unsafe_code)]

fn main() {
    eprintln!(
        "aisense-mcp {} — ainda em construção (Fase 05).",
        aisense_core::VERSION
    );
    std::process::exit(1);
}
