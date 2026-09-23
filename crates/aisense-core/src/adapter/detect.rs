//! Detecção de runtimes instalados (`docs/05`, campo `detect`).
//!
//! Roda o comando de detecção de cada adaptador com `stdin` fechado e timeout: uma CLI
//! que abre um prompt interativo não pode travar a tela de runtimes (risco da Fase 02).

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog::AdapterCatalog;
use super::model::{Adapter, AdapterProblem, DetectSpec};

pub const DETECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Valor de `command` que significa "o shell padrão do sistema".
pub const SHELL_PLACEHOLDER: &str = "$SHELL";

/// Valor de `command` que significa "o agente diz qual comando rodar" (adaptador
/// `custom`): o supervisor usa o comando configurado no agente.
pub const AGENT_COMMAND_PLACEHOLDER: &str = "$AGENT_COMMAND";

const VERSION_MAX_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum RuntimeStatus {
    /// `version` é a primeira linha que o `detect` imprimiu; sem `detect`, fica vazia.
    Available {
        path: String,
        version: Option<String>,
    },
    /// `reason` é para o usuário (em inglês, como as demais mensagens do core).
    Missing { reason: String },
    /// O comando é escolhido em cada agente; não há o que detectar antes.
    PerAgent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct RuntimeInfo {
    pub adapter: Adapter,
    pub status: RuntimeStatus,
}

/// O que a tela de runtimes mostra: cada adaptador com seu estado, e os arquivos que
/// não carregaram.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct RuntimeOverview {
    pub runtimes: Vec<RuntimeInfo>,
    pub problems: Vec<AdapterProblem>,
}

/// Resolve `$SHELL` para o shell do sistema; qualquer outro comando passa intacto.
pub fn resolve_command(command: &str) -> String {
    if command != SHELL_PLACEHOLDER {
        return command.to_owned();
    }
    if cfg!(windows) {
        return "powershell.exe".to_owned();
    }
    std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_owned())
}

/// Procura `program` no `PATH`, como o shell faria.
///
/// No Windows também tenta as extensões de `PATHEXT`: CLIs instaladas pelo npm
/// (`claude`, `codex`) são `claude.cmd`, e o `Command` da std só completa `.exe`.
pub fn which(program: &str) -> Option<PathBuf> {
    which_in(
        program,
        std::env::var_os("PATH"),
        std::env::var_os("PATHEXT"),
    )
}

/// Como [`which`], mas num `PATH` dado (o que o agente vai receber, não o do app).
pub(crate) fn which_in_path(program: &str, path: &OsString) -> Option<PathBuf> {
    which_in(program, Some(path.clone()), std::env::var_os("PATHEXT"))
}

fn which_in(program: &str, path: Option<OsString>, pathext: Option<OsString>) -> Option<PathBuf> {
    let candidate = Path::new(program);
    let extensions = executable_extensions(candidate, pathext);
    if candidate.components().count() > 1 || candidate.is_absolute() {
        return with_extensions(candidate, &extensions);
    }
    std::env::split_paths(&path?).find_map(|dir| with_extensions(&dir.join(program), &extensions))
}

fn executable_extensions(program: &Path, pathext: Option<OsString>) -> Vec<String> {
    if !cfg!(windows) || program.extension().is_some() {
        return vec![String::new()];
    }
    // Sem a entrada vazia de propósito: o npm instala, ao lado de `codex.cmd`, um
    // script `codex` sem extensão (para bash) que o Windows não executa. O próprio
    // `cmd.exe` só tenta os nomes com as extensões do PATHEXT.
    let list = pathext
        .and_then(|v| v.into_string().ok())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_owned());
    list.split(';')
        .filter(|e| !e.is_empty())
        .map(str::to_owned)
        .collect()
}

fn with_extensions(base: &Path, extensions: &[String]) -> Option<PathBuf> {
    extensions.iter().find_map(|ext| {
        let mut path = base.as_os_str().to_owned();
        path.push(ext);
        let path = PathBuf::from(path);
        path.is_file().then_some(path)
    })
}

/// Estado de um adaptador. Bloqueia por até `timeout`.
pub fn detect_runtime(adapter: &Adapter, timeout: Duration) -> RuntimeStatus {
    if adapter.command == AGENT_COMMAND_PLACEHOLDER {
        return RuntimeStatus::PerAgent;
    }
    let command = resolve_command(&adapter.command);
    let Some(detect) = &adapter.detect else {
        // Sem `detect`, basta o executável existir.
        return match which(&command) {
            Some(path) => RuntimeStatus::Available {
                path: path.display().to_string(),
                version: None,
            },
            None => not_found(&command),
        };
    };
    let program = resolve_command(&detect.command);
    let Some(path) = which(&program) else {
        return not_found(&program);
    };
    match run_with_timeout(&path, &detect.args, timeout) {
        Ok(output) if output.success => RuntimeStatus::Available {
            path: path.display().to_string(),
            version: first_line(&output.stdout).or_else(|| first_line(&output.stderr)),
        },
        Ok(output) => RuntimeStatus::Missing {
            reason: format!(
                "`{program}` exited with {}{}",
                output
                    .code
                    .map_or_else(|| "an error".to_owned(), |c| format!("code {c}")),
                first_line(&output.stderr).map_or_else(String::new, |l| format!(": {l}"))
            ),
        },
        Err(reason) => RuntimeStatus::Missing { reason },
    }
}

fn not_found(program: &str) -> RuntimeStatus {
    RuntimeStatus::Missing {
        reason: format!("`{program}` was not found in PATH"),
    }
}

struct Output {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_with_timeout(path: &Path, args: &[String], timeout: Duration) -> Result<Output, String> {
    let mut child = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("could not run `{}`: {err}", path.display()))?;

    // A saída é lida em paralelo: uma CLI que imprime mais que o buffer do pipe
    // travaria esperando alguém ler, e pareceria timeout.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "`{}` did not answer within {} s",
                    path.display(),
                    timeout.as_secs_f32()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(err) => return Err(format!("could not wait for `{}`: {err}", path.display())),
        }
    };
    Ok(Output {
        success: status.success(),
        code: status.code(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

fn first_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(VERSION_MAX_CHARS).collect())
}

type Detector = dyn Fn(&Adapter) -> RuntimeStatus + Send + Sync;

/// O que decide se um resultado em cache ainda vale: mudou o comando ou o `detect`,
/// detecta de novo.
#[derive(PartialEq)]
struct CacheKey {
    command: String,
    detect: Option<DetectSpec>,
}

impl CacheKey {
    fn of(adapter: &Adapter) -> Self {
        Self {
            command: adapter.command.clone(),
            detect: adapter.detect.clone(),
        }
    }
}

/// Catálogo atual + estado de cada runtime, em cache até alguém pedir `refresh`.
///
/// Compartilhado entre o comando da UI e o observador de arquivos.
pub struct RuntimeRegistry {
    inner: Mutex<Inner>,
    detector: Arc<Detector>,
}

struct Inner {
    catalog: AdapterCatalog,
    cache: HashMap<String, (CacheKey, RuntimeStatus)>,
}

impl RuntimeRegistry {
    pub fn new(catalog: AdapterCatalog) -> Self {
        Self::with_detector(catalog, |adapter| detect_runtime(adapter, DETECT_TIMEOUT))
    }

    pub fn with_detector(
        catalog: AdapterCatalog,
        detector: impl Fn(&Adapter) -> RuntimeStatus + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Mutex::new(Inner {
                catalog,
                cache: HashMap::new(),
            }),
            detector: Arc::new(detector),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // Nenhum código segura o lock enquanto pode entrar em pânico de forma que
        // deixe o estado pela metade, então um lock envenenado ainda é consistente.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn catalog(&self) -> AdapterCatalog {
        self.lock().catalog.clone()
    }

    pub fn adapter(&self, id: &str) -> Option<Adapter> {
        self.lock().catalog.get(id).cloned()
    }

    /// Troca o catálogo (hot-reload). O cache de quem não mudou é mantido.
    pub fn set_catalog(&self, catalog: AdapterCatalog) {
        let mut inner = self.lock();
        inner
            .cache
            .retain(|id, (key, _)| catalog.get(id).is_some_and(|a| CacheKey::of(a) == *key));
        inner.catalog = catalog;
    }

    /// Estado de todos os runtimes. Detecta, em paralelo, só o que não está em cache
    /// (ou tudo, com `refresh`). Bloqueia por até [`DETECT_TIMEOUT`].
    pub fn overview(&self, refresh: bool) -> RuntimeOverview {
        let pending: Vec<Adapter> = {
            let inner = self.lock();
            inner
                .catalog
                .adapters()
                .filter(|a| {
                    refresh
                        || !inner
                            .cache
                            .get(&a.id)
                            .is_some_and(|(key, _)| *key == CacheKey::of(a))
                })
                .cloned()
                .collect()
        };

        // Sem lock durante a detecção: ela pode levar segundos.
        let detected: Vec<(Adapter, RuntimeStatus)> = std::thread::scope(|scope| {
            let handles: Vec<_> = pending
                .into_iter()
                .map(|adapter| {
                    let detector = Arc::clone(&self.detector);
                    scope.spawn(move || {
                        let status = detector(&adapter);
                        (adapter, status)
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });

        let mut inner = self.lock();
        for (adapter, status) in detected {
            tracing::debug!(adapter = %adapter.id, ?status, "runtime detectado");
            inner
                .cache
                .insert(adapter.id.clone(), (CacheKey::of(&adapter), status));
        }
        let runtimes = inner
            .catalog
            .adapters()
            .map(|adapter| RuntimeInfo {
                adapter: adapter.clone(),
                status: inner.cache.get(&adapter.id).map_or_else(
                    // Só acontece se o catálogo trocou durante a detecção; o próximo
                    // pedido detecta.
                    || RuntimeStatus::Missing {
                        reason: "not checked yet".to_owned(),
                    },
                    |(_, status)| status.clone(),
                ),
            })
            .collect();
        RuntimeOverview {
            runtimes,
            problems: inner.catalog.problems().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::adapter::{parse_adapter, AdapterSource, Builtin};

    fn adapter(toml: &str) -> Adapter {
        parse_adapter(toml, "test.toml", AdapterSource::Builtin).unwrap()
    }

    /// Um detect que imprime `text` e termina com `code`, nos três sistemas.
    fn detect_printing(text: &str, code: i32) -> String {
        if cfg!(windows) {
            format!(
                "detect = {{ command = \"cmd\", args = [\"/C\", \"echo {text}& exit /b {code}\"] }}"
            )
        } else {
            format!(
                "detect = {{ command = \"sh\", args = [\"-c\", \"echo {text}; exit {code}\"] }}"
            )
        }
    }

    #[test]
    fn available_runtime_reports_its_version() {
        let a = adapter(&format!(
            "id = \"x\"\nname = \"X\"\ncommand = \"x\"\n{}\n",
            detect_printing("x-cli 1.2.3", 0)
        ));
        match detect_runtime(&a, DETECT_TIMEOUT) {
            RuntimeStatus::Available { version, .. } => {
                assert_eq!(version.as_deref(), Some("x-cli 1.2.3"));
            }
            other => panic!("esperava disponível, veio {other:?}"),
        }
    }

    #[test]
    fn failing_detect_is_missing_with_the_exit_code() {
        let a = adapter(&format!(
            "id = \"x\"\nname = \"X\"\ncommand = \"x\"\n{}\n",
            detect_printing("nope", 3)
        ));
        let RuntimeStatus::Missing { reason } = detect_runtime(&a, DETECT_TIMEOUT) else {
            panic!("esperava indisponível");
        };
        assert!(reason.contains("code 3"), "{reason}");
    }

    #[test]
    fn unknown_command_is_missing() {
        let a = adapter(
            "id = \"x\"\nname = \"X\"\ncommand = \"x\"\n\
             detect = { command = \"aisense-no-such-cli-42\", args = [\"--version\"] }\n",
        );
        let RuntimeStatus::Missing { reason } = detect_runtime(&a, DETECT_TIMEOUT) else {
            panic!("esperava indisponível");
        };
        assert!(reason.contains("not found"), "{reason}");
    }

    #[test]
    fn a_hanging_detect_times_out() {
        let slow = if cfg!(windows) {
            "detect = { command = \"ping\", args = [\"-n\", \"30\", \"127.0.0.1\"] }"
        } else {
            "detect = { command = \"sleep\", args = [\"30\"] }"
        };
        let a = adapter(&format!(
            "id = \"x\"\nname = \"X\"\ncommand = \"x\"\n{slow}\n"
        ));
        let started = Instant::now();
        let status = detect_runtime(&a, Duration::from_millis(300));
        assert!(started.elapsed() < Duration::from_secs(10));
        let RuntimeStatus::Missing { reason } = status else {
            panic!("esperava timeout");
        };
        assert!(reason.contains("did not answer"), "{reason}");
    }

    #[test]
    fn builtin_shell_is_available_everywhere() {
        let catalog = AdapterCatalog::load_from(crate::adapter::BUILTIN_ADAPTERS, None);
        let shell = catalog.get("shell").unwrap();
        assert!(
            matches!(
                detect_runtime(shell, DETECT_TIMEOUT),
                RuntimeStatus::Available { .. }
            ),
            "o shell precisa funcionar em qualquer máquina"
        );
    }

    #[test]
    fn custom_adapter_needs_no_detection() {
        let catalog = AdapterCatalog::load_from(crate::adapter::BUILTIN_ADAPTERS, None);
        let custom = catalog.get("custom").unwrap();
        assert_eq!(
            detect_runtime(custom, DETECT_TIMEOUT),
            RuntimeStatus::PerAgent
        );
    }

    /// Guarda contra flag errada num adaptador embutido: em toda máquina onde a CLI
    /// existe, o `detect` precisa responder com sucesso (aceite da F02-07).
    #[test]
    fn installed_builtin_clis_answer_their_detect() {
        let catalog = AdapterCatalog::load_from(crate::adapter::BUILTIN_ADAPTERS, None);
        for adapter in catalog.adapters() {
            let Some(detect) = &adapter.detect else {
                continue;
            };
            if which(&resolve_command(&detect.command)).is_none() {
                continue; // não instalada aqui
            }
            // Timeout generoso: CLIs em Node demoram a subir a frio.
            let status = detect_runtime(adapter, Duration::from_secs(20));
            match &status {
                RuntimeStatus::Available { .. } => {}
                // Lentidão da máquina não diz nada sobre a flag; só erro de execução diz.
                RuntimeStatus::Missing { reason } if reason.contains("did not answer") => {
                    eprintln!(
                        "aviso: {} não respondeu a tempo; flag não verificada",
                        adapter.id
                    );
                }
                _ => panic!(
                    "{} está instalado mas o detect falhou: {status:?}",
                    adapter.id
                ),
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn which_finds_cmd_shims_through_pathext() {
        let dir = std::env::temp_dir().join(format!("aisense-which-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("fakecli.cmd"), "@echo 1.0").unwrap();
        // O script sem extensão que o npm instala para bash: não é executável no Windows.
        std::fs::write(dir.join("fakecli"), "#!/bin/sh\necho 1.0\n").unwrap();
        let found = which_in(
            "fakecli",
            Some(dir.clone().into_os_string()),
            Some(".EXE;.CMD".into()),
        );
        let _ = std::fs::remove_dir_all(&dir);
        // A extensão vem de PATHEXT (maiúscula); o sistema de arquivos não diferencia.
        let name = found
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        assert_eq!(name, "fakecli.cmd");
    }

    const ONE: Builtin = (
        "one.toml",
        "id = \"one\"\nname = \"One\"\ncommand = \"one\"\n",
    );
    const TWO: Builtin = (
        "two.toml",
        "id = \"two\"\nname = \"Two\"\ncommand = \"two\"\n",
    );

    fn counting_registry(builtins: &[Builtin]) -> (RuntimeRegistry, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let registry =
            RuntimeRegistry::with_detector(AdapterCatalog::load_from(builtins, None), move |a| {
                counter.fetch_add(1, Ordering::SeqCst);
                RuntimeStatus::Available {
                    path: a.command.clone(),
                    version: None,
                }
            });
        (registry, calls)
    }

    #[test]
    fn registry_caches_until_refresh() {
        let (registry, calls) = counting_registry(&[ONE, TWO]);
        assert_eq!(registry.overview(false).runtimes.len(), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        registry.overview(false);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "segunda chamada deveria vir do cache"
        );
        registry.overview(true);
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn changing_an_adapter_invalidates_only_its_cache() {
        let (registry, calls) = counting_registry(&[ONE, TWO]);
        registry.overview(false);
        const TWO_EDITED: Builtin = (
            "two.toml",
            "id = \"two\"\nname = \"Two\"\ncommand = \"two-v2\"\n",
        );
        registry.set_catalog(AdapterCatalog::load_from(&[ONE, TWO_EDITED], None));
        let overview = registry.overview(false);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        let two = overview
            .runtimes
            .iter()
            .find(|r| r.adapter.id == "two")
            .unwrap();
        assert_eq!(
            two.status,
            RuntimeStatus::Available {
                path: "two-v2".into(),
                version: None
            }
        );
    }
}
