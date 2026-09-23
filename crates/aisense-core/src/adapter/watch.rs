//! Hot-reload: salvou um TOML em `~/.aisense/adapters/`, o catálogo é recarregado.
//!
//! Agentes já rodando não são afetados — continuam com o adaptador com que subiram
//! até reiniciarem (`docs/05`, "Onde ficam").

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::catalog::{AdapterCatalog, Builtin, BUILTIN_ADAPTERS};

/// Editores salvam em rajadas (arquivo temporário, rename, chmod). Esperamos este
/// silêncio antes de recarregar, para o callback não ver um arquivo pela metade.
const DEBOUNCE: Duration = Duration::from_millis(200);

#[derive(Debug, thiserror::Error)]
pub enum AdapterWatchError {
    #[error("could not create the adapters folder {path}: {source}")]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[error("could not watch the adapters folder: {0}")]
    Watch(#[from] notify::Error),
    #[error("could not start the watcher thread: {0}")]
    Thread(std::io::Error),
}

/// Observa a pasta enquanto viver. Soltar o valor para o observador e encerra a thread.
pub struct AdapterWatcher {
    // Ordem importa: o `watcher` cai primeiro, fecha o canal e a thread termina.
    _watcher: RecommendedWatcher,
    _thread: JoinHandle<()>,
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
        std::fs::create_dir_all(dir).map_err(|source| AdapterWatchError::CreateDir {
            path: dir.display().to_string(),
            source,
        })?;
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(tx)?;
        watcher.watch(dir, RecursiveMode::NonRecursive)?;

        let dir: PathBuf = dir.to_path_buf();
        let thread = std::thread::Builder::new()
            .name("aisense-adapter-watch".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    let mut relevant = is_relevant(&first);
                    loop {
                        match rx.recv_timeout(DEBOUNCE) {
                            Ok(event) => relevant |= is_relevant(&event),
                            Err(mpsc::RecvTimeoutError::Timeout) => break,
                            Err(mpsc::RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    if relevant {
                        tracing::debug!(dir = %dir.display(), "recarregando adaptadores");
                        on_change(AdapterCatalog::load_from(builtins, Some(&dir)));
                    }
                }
            })
            .map_err(AdapterWatchError::Thread)?;

        Ok(Self {
            _watcher: watcher,
            _thread: thread,
        })
    }
}

/// Só interessa mudança em `.toml`; erro do observador é registrado e força uma
/// recarga, porque não dá para saber o que se perdeu.
fn is_relevant(event: &notify::Result<notify::Event>) -> bool {
    match event {
        Ok(event) => {
            !event.kind.is_access()
                && event
                    .paths
                    .iter()
                    .any(|p| p.extension().is_some_and(|ext| ext == "toml"))
        }
        Err(err) => {
            tracing::warn!(%err, "falha no observador de adaptadores");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

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
