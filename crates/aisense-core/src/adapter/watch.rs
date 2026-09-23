//! Hot-reload: salvou um TOML em `~/.aisense/adapters/`, o catálogo é recarregado.
//!
//! Agentes já rodando não são afetados — continuam com o adaptador com que subiram
//! até reiniciarem (`docs/05`, "Onde ficam").

use std::path::Path;

use super::catalog::{AdapterCatalog, Builtin, BUILTIN_ADAPTERS};
use crate::fswatch::{DirWatcher, WatchError};

pub type AdapterWatchError = WatchError;

/// Observa a pasta enquanto viver. Soltar o valor para o observador.
pub struct AdapterWatcher {
    _inner: DirWatcher,
}

impl AdapterWatcher {
    /// Cria a pasta se preciso (é o lugar onde o usuário vai salvar) e chama
    /// `on_change` com o catálogo novo a cada mudança.
    pub fn spawn(
        dir: &Path,
        on_change: impl Fn(AdapterCatalog) + Send + 'static,
    ) -> Result<Self, AdapterWatchError> {
        Self::spawn_with(BUILTIN_ADAPTERS, dir, on_change)
    }

    pub fn spawn_with(
        builtins: &'static [Builtin],
        dir: &Path,
        on_change: impl Fn(AdapterCatalog) + Send + 'static,
    ) -> Result<Self, AdapterWatchError> {
        let inner = DirWatcher::spawn(dir, false, "aisense-adapter-watch", is_toml, move |dir| {
            on_change(AdapterCatalog::load_from(builtins, Some(dir)));
        })?;
        Ok(Self { _inner: inner })
    }
}

/// Só interessa mudança em `.toml`.
fn is_toml(event: &notify::Event) -> bool {
    event
        .paths
        .iter()
        .any(|p| p.extension().is_some_and(|ext| ext == "toml"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn saving_a_file_reloads_the_catalog() {
        let dir = std::env::temp_dir().join(format!("aisense-watch-{}", ulid::Ulid::new()));
        let (tx, rx) = mpsc::channel();
        let watcher = AdapterWatcher::spawn_with(&[], &dir, move |catalog| {
            let _ = tx.send(catalog);
        })
        .unwrap();

        std::fs::write(
            dir.join("hot.toml"),
            "id = \"hot\"\nname = \"Hot\"\ncommand = \"hot\"\n",
        )
        .unwrap();

        // Um mesmo save pode render mais de uma recarga; basta uma ver o arquivo.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut found = false;
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(catalog) if catalog.get("hot").is_some() => {
                    found = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        drop(watcher);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(found, "o catálogo não foi recarregado depois do save");
    }
}
