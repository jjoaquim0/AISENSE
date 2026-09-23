//! Conjunto de adaptadores disponíveis: embutidos + `~/.aisense/adapters/*.toml`.

use std::collections::BTreeMap;
use std::path::Path;

use super::file::parse_adapter;
use super::model::{Adapter, AdapterProblem, AdapterSource};

/// Um adaptador que acompanha o binário: nome do arquivo e conteúdo.
pub type Builtin = (&'static str, &'static str);

/// Adaptadores mantidos pelo projeto (`adapters/` na raiz do repositório).
pub const BUILTIN_ADAPTERS: &[Builtin] = &[(
    "shell.toml",
    include_str!("../../../../adapters/shell.toml"),
)];

/// Resultado de uma carga. Sempre existe: arquivo ruim vira `problem`, nunca erro
/// que impede o app de subir.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdapterCatalog {
    adapters: BTreeMap<String, Adapter>,
    problems: Vec<AdapterProblem>,
}

impl AdapterCatalog {
    /// Embutidos + pasta do usuário. Pasta inexistente é o caso comum, não erro.
    pub fn load(user_dir: &Path) -> Self {
        Self::load_from(BUILTIN_ADAPTERS, Some(user_dir))
    }

    pub fn load_from(builtins: &[Builtin], user_dir: Option<&Path>) -> Self {
        let mut catalog = Self::default();
        for (name, source) in builtins {
            let path = format!("builtin:{name}");
            match parse_adapter(source, &path, AdapterSource::Builtin) {
                Ok(adapter) => {
                    catalog.adapters.insert(adapter.id.clone(), adapter);
                }
                Err(problem) => catalog.report(problem),
            }
        }
        if let Some(dir) = user_dir {
            catalog.load_user_dir(dir);
        }
        catalog
    }

    fn load_user_dir(&mut self, dir: &Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) => {
                self.report(AdapterProblem {
                    path: dir.display().to_string(),
                    line: None,
                    column: None,
                    message: format!("could not read the adapters folder: {err}"),
                });
                return;
            }
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        // Ordem estável: com dois arquivos de mesmo `id`, vence sempre o mesmo.
        files.sort();

        let mut seen_in_user_dir: BTreeMap<String, String> = BTreeMap::new();
        for path in files {
            let shown = path.display().to_string();
            let source = match std::fs::read_to_string(&path) {
                Ok(source) => source,
                Err(err) => {
                    self.report(AdapterProblem {
                        path: shown,
                        line: None,
                        column: None,
                        message: format!("could not read the file: {err}"),
                    });
                    continue;
                }
            };
            let origin = AdapterSource::User {
                path: shown.clone(),
            };
            match parse_adapter(&source, &shown, origin) {
                Ok(adapter) => {
                    if let Some(first) = seen_in_user_dir.get(&adapter.id) {
                        self.report(AdapterProblem {
                            path: shown,
                            line: None,
                            column: None,
                            message: format!(
                                "id {:?} is already defined in {first}; this file was ignored",
                                adapter.id
                            ),
                        });
                        continue;
                    }
                    seen_in_user_dir.insert(adapter.id.clone(), shown);
                    // Precedência do usuário: sobrescreve o embutido de mesmo `id`.
                    self.adapters.insert(adapter.id.clone(), adapter);
                }
                Err(problem) => self.report(problem),
            }
        }
    }

    fn report(&mut self, problem: AdapterProblem) {
        tracing::warn!(%problem, "adaptador ignorado");
        self.problems.push(problem);
    }

    pub fn get(&self, id: &str) -> Option<&Adapter> {
        self.adapters.get(id)
    }

    /// Em ordem de `id`.
    pub fn adapters(&self) -> impl Iterator<Item = &Adapter> {
        self.adapters.values()
    }

    pub fn problems(&self) -> &[AdapterProblem] {
        &self.problems
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::path::PathBuf;

    use super::*;

    const DEMO: Builtin = (
        "demo.toml",
        "id = \"demo\"\nname = \"Demo\"\ncommand = \"demo\"\n",
    );

    /// Pasta temporária própria, apagada ao sair do teste.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("aisense-{tag}-{}", ulid::Ulid::new()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn write(&self, name: &str, content: &str) {
            std::fs::write(self.0.join(name), content).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn every_builtin_adapter_is_valid() {
        let catalog = AdapterCatalog::load_from(BUILTIN_ADAPTERS, None);
        assert_eq!(catalog.problems(), &[]);
        assert_eq!(catalog.adapters().count(), BUILTIN_ADAPTERS.len());
        assert!(catalog.get("shell").is_some());
    }

    #[test]
    fn missing_user_dir_is_not_a_problem() {
        let catalog = AdapterCatalog::load_from(&[DEMO], Some(Path::new("/no/such/dir/aisense")));
        assert!(catalog.problems().is_empty());
        assert!(catalog.get("demo").is_some());
    }

    #[test]
    fn user_file_overrides_builtin_with_same_id() {
        let dir = TempDir::new("override");
        dir.write(
            "mine.toml",
            "id = \"demo\"\nname = \"Minha Demo\"\ncommand = \"demo2\"\n",
        );
        let catalog = AdapterCatalog::load_from(&[DEMO], Some(&dir.0));
        let demo = catalog.get("demo").unwrap();
        assert_eq!(demo.command, "demo2");
        assert!(matches!(demo.source, AdapterSource::User { .. }));
    }

    #[test]
    fn broken_file_is_reported_and_others_still_load() {
        let dir = TempDir::new("broken");
        dir.write(
            "a-good.toml",
            "id = \"good\"\nname = \"Good\"\ncommand = \"g\"\n",
        );
        dir.write("b-bad.toml", "id = \"bad\"\nname = \n");
        dir.write("notes.txt", "not an adapter");
        let catalog = AdapterCatalog::load_from(&[DEMO], Some(&dir.0));

        assert!(catalog.get("good").is_some());
        assert!(catalog.get("demo").is_some());
        assert_eq!(catalog.problems().len(), 1);
        let problem = &catalog.problems()[0];
        assert!(problem.path.ends_with("b-bad.toml"), "{problem}");
        assert_eq!(problem.line, Some(2));
    }

    #[test]
    fn duplicate_id_in_user_dir_keeps_the_first_file() {
        let dir = TempDir::new("dup");
        dir.write("a.toml", "id = \"x\"\nname = \"A\"\ncommand = \"a\"\n");
        dir.write("b.toml", "id = \"x\"\nname = \"B\"\ncommand = \"b\"\n");
        let catalog = AdapterCatalog::load_from(&[], Some(&dir.0));
        assert_eq!(catalog.get("x").unwrap().name, "A");
        assert_eq!(catalog.problems().len(), 1);
        assert!(catalog.problems()[0].path.ends_with("b.toml"));
    }
}
