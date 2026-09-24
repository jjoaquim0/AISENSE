//! O que a CLI faz sozinha, no terminal do agente (F05-13): `run`, `commands` e `bench`.
//! `run` só aceita nomes do `aisense.toml`; o resultado vai para a linha do tempo.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aisense_core::bench::local::{self as bench, BenchOpError};
use aisense_core::project::run::{resolve, run_named, summary, RunError};
use aisense_core::project::{load_project, ProjectConfig, ProjectLookup};
use aisense_ipc::cli::{BenchAction, Command};
use aisense_ipc::{Client, Request};

use crate::exit;

/// O diretório do agente: o que o supervisor abriu (a bancada, quando houver).
fn workdir() -> PathBuf {
    std::env::var_os("AISENSE_WORKDIR")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn project() -> Result<Option<ProjectConfig>, String> {
    match load_project(&workdir()) {
        ProjectLookup::Found { config, .. } => Ok(Some(config)),
        ProjectLookup::Missing { .. } => Ok(None),
        ProjectLookup::Invalid { problem } => Err(format!(
            "{}{}: {}",
            problem.path,
            problem.line.map(|l| format!(":{l}")).unwrap_or_default(),
            problem.message
        )),
    }
}

fn run_error(e: &RunError) -> u8 {
    eprintln!("erro: {e}");
    if let Some(hint) = e.hint() {
        eprintln!("dica: {hint}");
    }
    exit::USAGE
}

fn bench_error(e: &BenchOpError) -> u8 {
    eprintln!("erro: {e}");
    if let Some(hint) = e.hint() {
        eprintln!("dica: {hint}");
    }
    exit::USAGE
}

pub async fn handle(command: &Command, json: bool) -> Option<u8> {
    Some(match command {
        Command::Commands => {
            let config = match project() {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("erro: {e}");
                    return Some(exit::USAGE);
                }
            };
            let commands = config.map(|c| c.commands).unwrap_or_default();
            if json {
                println!("{}", serde_json::to_string(&commands).unwrap_or_default());
            } else if commands.is_empty() {
                println!("Nenhum comando definido: peça ao humano para aceitar o aisense.toml.");
            } else {
                for (name, cmd) in &commands {
                    println!("aisense run {name:<12} {}", cmd.run);
                }
            }
            exit::OK
        }
        Command::Run { name } => {
            let config = match project() {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("erro: {e}");
                    return Some(exit::USAGE);
                }
            };
            let command = match resolve(config.as_ref(), name) {
                Ok(command) => command.clone(),
                Err(e) => return Some(run_error(&e)),
            };
            // Com `--json`, a saída do comando vai para stderr: stdout fica só com o JSON.
            let echo: Arc<Mutex<dyn std::io::Write + Send>> = if json {
                Arc::new(Mutex::new(std::io::stderr()))
            } else {
                Arc::new(Mutex::new(std::io::stdout()))
            };
            let (dir, name2) = (workdir(), name.clone());
            let report =
                match tokio::task::spawn_blocking(move || run_named(&dir, &name2, &command, echo))
                    .await
                {
                    Ok(Ok(report)) => report,
                    Ok(Err(e)) => return Some(run_error(&e)),
                    Err(e) => {
                        eprintln!("erro: {e}");
                        return Some(exit::USAGE);
                    }
                };
            record(&summary(&report)).await;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "command": report.command,
                        "exit_code": report.exit_code,
                        "duration_ms": report.duration_ms,
                        "timed_out": report.timed_out,
                    })
                );
            } else if report.timed_out {
                eprintln!("aisense: `{name}` estourou o tempo e foi interrompido.");
            }
            // O exit code do próprio comando; timeout é 2, como no resto da CLI.
            match (report.timed_out, report.exit_code) {
                (true, _) => exit::TIMEOUT,
                (false, Some(code)) => u8::try_from(code).unwrap_or(1),
                (false, None) => 1,
            }
        }
        Command::Bench(action) => {
            let dir = workdir();
            let action = *action;
            let result = tokio::task::spawn_blocking(move || -> Result<String, BenchOpError> {
                Ok(match action {
                    BenchAction::Status => {
                        let s = bench::status(&dir)?;
                        if json {
                            serde_json::to_string(&s).unwrap_or_default()
                        } else {
                            let mut out = format!(
                                "bancada: {}\nbranch: {}\n",
                                s.path,
                                s.branch.as_deref().unwrap_or("(sem branch)")
                            );
                            match &s.base {
                                Some(base) => out.push_str(&format!(
                                    "base: {base} · {} à frente, {} atrás\n",
                                    s.ahead, s.behind
                                )),
                                None => out
                                    .push_str("diretório compartilhado da equipe (sem bancada)\n"),
                            }
                            out.push_str(&format!(
                                "{} arquivo(s) com mudança pendente\n",
                                s.pending.len()
                            ));
                            for line in &s.pending {
                                out.push_str(&format!("  {line}\n"));
                            }
                            out
                        }
                    }
                    BenchAction::Sync => {
                        let out = bench::sync(&dir)?;
                        format!(
                            "{}\n",
                            if out.is_empty() {
                                "já estava em dia"
                            } else {
                                &out
                            }
                        )
                    }
                    BenchAction::Diff => bench::diff(&dir)? + "\n",
                    BenchAction::Publish => {
                        let p = bench::publish(&dir)?;
                        if json {
                            serde_json::to_string(&p).unwrap_or_default()
                        } else {
                            match p.compare_url {
                                Some(url) => format!("publicado {}\nabra o PR: {url}\n", p.branch),
                                None => format!("publicado {}\n", p.branch),
                            }
                        }
                    }
                    BenchAction::List => {
                        let all = bench::list(&dir)?;
                        if json {
                            serde_json::to_string(&all).unwrap_or_default()
                        } else {
                            all.iter()
                                .map(|w| {
                                    format!(
                                        "{}{}  {}\n",
                                        if w.main { "* " } else { "  " },
                                        w.branch.as_deref().unwrap_or("(sem branch)"),
                                        w.path
                                    )
                                })
                                .collect()
                        }
                    }
                })
            })
            .await;
            match result {
                Ok(Ok(text)) => {
                    print!("{text}");
                    if json {
                        println!();
                    }
                    exit::OK
                }
                Ok(Err(e)) => bench_error(&e),
                Err(e) => {
                    eprintln!("erro: {e}");
                    exit::USAGE
                }
            }
        }
        _ => return None,
    })
}

/// Deixa o resultado de `run` na linha do tempo. Sem AISENSE por perto, só não registra.
async fn record(text: &str) {
    let (Ok(socket), Ok(token)) = (
        std::env::var("AISENSE_SOCKET"),
        std::env::var("AISENSE_TOKEN"),
    ) else {
        return;
    };
    if let Ok(mut client) = Client::connect(&socket, &token).await {
        let _ = client
            .call(&Request::Note {
                body: text.to_owned(),
            })
            .await;
    }
}
