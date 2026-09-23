//! Observar uma pasta de configuração e recarregar quando algo muda (adaptadores,
//! skills). Editores salvam em rajadas — arquivo temporário, rename, chmod —, então a
//! recarga espera um silêncio antes de rodar, para não ver um arquivo pela metade.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

pub const DEBOUNCE: Duration = Duration::from_millis(200);

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("could not create the folder {path}: {source}")]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[error("could not watch the folder: {0}")]
    Watch(#[from] notify::Error),
    #[error("could not start the watcher thread: {0}")]
    Thread(std::io::Error),
}

/// Observa enquanto viver. Soltar o valor para o observador e encerra a thread.
pub struct DirWatcher {
    // Ordem importa: o `watcher` cai primeiro, fecha o canal e a thread termina.
    _watcher: RecommendedWatcher,
    _thread: JoinHandle<()>,
}

impl DirWatcher {
    /// Cria a pasta se preciso (é onde o usuário vai salvar) e chama `reload` depois de
    /// cada rajada de eventos em que `relevant` aceitou ao menos um. Erro do observador
    /// conta como relevante: não dá para saber o que se perdeu.
    pub fn spawn(
        dir: &Path,
        recursive: bool,
        thread_name: &str,
        relevant: fn(&notify::Event) -> bool,
        mut reload: impl FnMut(&Path) + Send + 'static,
    ) -> Result<Self, WatchError> {
        std::fs::create_dir_all(dir).map_err(|source| WatchError::CreateDir {
            path: dir.display().to_string(),
            source,
        })?;
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(tx)?;
        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        watcher.watch(dir, mode)?;

        let dir: PathBuf = dir.to_path_buf();
        let check = move |event: &notify::Result<notify::Event>| match event {
            Ok(event) => !event.kind.is_access() && relevant(event),
            Err(err) => {
                tracing::warn!(%err, dir = %dir_label(event), "falha no observador de pasta");
                true
            }
        };
        let thread = std::thread::Builder::new()
            .name(thread_name.to_owned())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    let mut changed = check(&first);
                    loop {
                        match rx.recv_timeout(DEBOUNCE) {
                            Ok(event) => changed |= check(&event),
                            Err(mpsc::RecvTimeoutError::Timeout) => break,
                            Err(mpsc::RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    if changed {
                        tracing::debug!(dir = %dir.display(), "recarregando");
                        reload(&dir);
                    }
                }
            })
            .map_err(WatchError::Thread)?;

        Ok(Self {
            _watcher: watcher,
            _thread: thread,
        })
    }
}

fn dir_label(event: &notify::Result<notify::Event>) -> String {
    match event {
        Ok(event) => event
            .paths
            .first()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        Err(err) => err
            .paths
            .first()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }
}
