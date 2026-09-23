//! Hot-reload da biblioteca: salvou um `SKILL.md` em `~/.aisense/skills/`, o catálogo é
//! recarregado. Agentes já rodando não mudam — skill não se aplica a quente (`docs/06`).

use std::path::Path;

use super::catalog::{BuiltinSkill, SkillCatalog, BUILTIN_SKILLS};
use crate::fswatch::{DirWatcher, WatchError};

/// Observa a pasta enquanto viver. Soltar o valor para o observador.
pub struct SkillWatcher {
    _inner: DirWatcher,
}

impl SkillWatcher {
    pub fn spawn(
        dir: &Path,
        on_change: impl Fn(SkillCatalog) + Send + 'static,
    ) -> Result<Self, WatchError> {
        Self::spawn_with(BUILTIN_SKILLS, dir, on_change)
    }

    pub fn spawn_with(
        builtins: &'static [BuiltinSkill],
        dir: &Path,
        on_change: impl Fn(SkillCatalog) + Send + 'static,
    ) -> Result<Self, WatchError> {
        // Recursivo: cada skill é uma pasta, e o que muda é o `SKILL.md` lá dentro.
        let inner = DirWatcher::spawn(
            dir,
            true,
            "aisense-skill-watch",
            touches_a_skill,
            move |dir| {
                on_change(SkillCatalog::load_from(builtins, Some(dir)));
            },
        )?;
        Ok(Self { _inner: inner })
    }
}

/// Um `SKILL.md`, ou uma pasta inteira criada, removida ou renomeada.
fn touches_a_skill(event: &notify::Event) -> bool {
    event.paths.iter().any(|p| {
        p.file_name().is_some_and(|n| n == super::SKILL_FILE)
            || (!event.kind.is_modify() && p.extension().is_none())
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::*;

    /// Espera até um catálogo recarregado satisfazer `ok`.
    fn wait_for(rx: &mpsc::Receiver<SkillCatalog>, ok: impl Fn(&SkillCatalog) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(catalog) if ok(&catalog) => return true,
                Ok(_) => continue,
                Err(_) => return false,
            }
        }
        false
    }

    #[test]
    fn editar_um_skill_md_recarrega_a_biblioteca() {
        let dir = std::env::temp_dir().join(format!("aisense-skills-{}", ulid::Ulid::new()));
        let (tx, rx) = mpsc::channel();
        let watcher = SkillWatcher::spawn_with(&[], &dir, move |catalog| {
            let _ = tx.send(catalog);
        })
        .unwrap();

        let folder = dir.join("quente");
        std::fs::create_dir_all(&folder).unwrap();
        let write = |description: &str| {
            std::fs::write(
                folder.join("SKILL.md"),
                format!("---\nname: quente\ndescription: {description}\n---\ncorpo\n"),
            )
            .unwrap();
        };
        write("Primeira versão.");
        let created = wait_for(&rx, |c| c.get("quente").is_some());
        write("Segunda versão.");
        let edited = wait_for(&rx, |c| {
            c.get("quente")
                .is_some_and(|s| s.description == "Segunda versão.")
        });
        std::fs::remove_dir_all(&folder).unwrap();
        let removed = wait_for(&rx, |c| c.get("quente").is_none());

        drop(watcher);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(created, "a skill nova não apareceu");
        assert!(edited, "a edição não recarregou");
        assert!(removed, "a remoção não recarregou");
    }
}
