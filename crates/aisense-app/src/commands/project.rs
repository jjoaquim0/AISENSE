//! Comandos do `aisense.toml` (`docs/17`).

use std::path::PathBuf;

use aisense_core::project::{load_project, parse_project_config, ProjectLookup, FILE_NAME};
use aisense_core::CommandError;

/// O que o app sabe dos comandos do diretório: arquivo válido, inválido (com a linha)
/// ou ausente (com a proposta, se algo foi reconhecido). Lê disco: fora da thread da UI.
#[tauri::command]
pub async fn project_lookup(workdir: String) -> Result<ProjectLookup, CommandError> {
    tauri::async_runtime::spawn_blocking(move || load_project(&PathBuf::from(workdir)))
        .await
        .map_err(|e| CommandError::new("project_lookup_failed", e.to_string(), None))
}

/// Grava o `aisense.toml` que o usuário revisou e aceitou. Nunca sobrescreve um que já
/// exista — trocar um arquivo versionado sem ver o diff não é papel do app.
#[tauri::command]
pub async fn project_accept(
    workdir: String,
    content: String,
) -> Result<ProjectLookup, CommandError> {
    let dir = PathBuf::from(workdir);
    let path = dir.join(FILE_NAME);
    if let Err(problem) = parse_project_config(&content, &path.display().to_string()) {
        return Err(CommandError::new(
            "invalid_project_config",
            problem.to_string(),
            Some("Corrija o conteúdo antes de aceitar.".to_owned()),
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                let (code, hint) = if e.kind() == std::io::ErrorKind::AlreadyExists {
                    (
                        "already_exists",
                        "Já existe um aisense.toml neste diretório.",
                    )
                } else {
                    ("write_failed", "Verifique as permissões do diretório.")
                };
                CommandError::new(code, e.to_string(), Some(hint.to_owned()))
            })?;
        file.write_all(content.as_bytes())
            .map_err(|e| CommandError::new("write_failed", e.to_string(), None))?;
        tracing::info!(path = %path.display(), "aisense.toml criado a partir da proposta");
        Ok(load_project(&dir))
    })
    .await
    .map_err(|e| CommandError::new("write_failed", e.to_string(), None))?
}
