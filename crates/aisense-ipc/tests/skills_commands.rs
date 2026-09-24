//! Todo comando `aisense ...` citado nas skills embutidas precisa existir na CLI: uma skill que
//! ensina um comando errado faz o agente errar em toda sessão (F07-01).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

/// Separa como o shell faria (aspas duplas agrupam).
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[test]
fn comandos_das_skills_existem_na_cli() {
    let skills = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills");
    let mut checked = 0;
    for entry in std::fs::read_dir(&skills).unwrap() {
        let file = entry.unwrap().path().join("SKILL.md");
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (i, span) in text.split('`').enumerate() {
            // Trechos de código inline são os ímpares.
            if i % 2 == 0 || !span.starts_with("aisense ") {
                continue;
            }
            let line = span
                .replace("<id>", "tsk_ABC123")
                .replace("<sha>", "a1b2c3d")
                .replace("<nome>", "api")
                .replace("<objetivo>", "oauth");
            let argv: Vec<String> = words(&line).into_iter().skip(1).collect();
            // Menção curta (`aisense broadcast`) pode omitir o texto; comando, ação ou opção
            // inventados, nunca.
            if let Err(error) = aisense_ipc::cli::parse(&argv) {
                let wrong = error.contains("desconhecid")
                    || error.contains("inesperado")
                    || error.contains("espera")
                    || argv.len() > 2;
                assert!(
                    !wrong,
                    "{}: `{span}` não é um comando válido: {error}",
                    file.display()
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 20, "só {checked} comandos encontrados");
}
