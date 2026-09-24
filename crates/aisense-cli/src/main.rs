//! Binário `aisense`: a interface que os agentes usam de dentro do próprio terminal.
//!
//! Precisa iniciar em poucos milissegundos — é executado a cada mensagem enviada.
//! Toda operação real vai pelo socket do barramento; este binário nunca toca o banco
//! (ver `docs/02-arquitetura.md` e ADR 0004).
#![forbid(unsafe_code)]

mod args;
mod render;

use std::process::ExitCode;

use aisense_ipc::{Client, ClientError, Request, Response};

use crate::args::{parse, Command, Parsed};

/// Códigos de saída contratados com as IAs. Ver `docs/07-barramento-comunicacao.md`.
pub mod exit {
    pub const OK: u8 = 0;
    pub const USAGE: u8 = 1;
    pub const TIMEOUT: u8 = 2;
    pub const UNAVAILABLE: u8 = 3;
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let parsed = match parse(&argv) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("erro: {message}");
            eprintln!("Veja: aisense --help");
            return ExitCode::from(exit::USAGE);
        }
    };
    match parsed.command {
        Command::Help => {
            print!("{}", args::HELP);
            return ExitCode::from(exit::OK);
        }
        Command::Version => {
            println!("aisense {}", aisense_core::VERSION);
            return ExitCode::from(exit::OK);
        }
        _ => {}
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("erro: {e}");
            return ExitCode::from(exit::UNAVAILABLE);
        }
    };
    ExitCode::from(runtime.block_on(run(parsed)))
}

async fn run(parsed: Parsed) -> u8 {
    let (Ok(socket), Ok(token)) = (
        std::env::var("AISENSE_SOCKET"),
        std::env::var("AISENSE_TOKEN"),
    ) else {
        eprintln!("erro: este terminal não foi aberto pelo AISENSE (AISENSE_SOCKET/AISENSE_TOKEN ausentes).");
        eprintln!("dica: rode o comando no terminal de um agente da equipe.");
        return exit::UNAVAILABLE;
    };
    let mut client = match Client::connect(&socket, &token).await {
        Ok(client) => client,
        Err(ClientError::Refused(response)) => return fail(&response, parsed.json),
        Err(e) => {
            eprintln!("erro: {e}");
            eprintln!("dica: o AISENSE está aberto? O barramento só funciona com o app rodando.");
            return exit::UNAVAILABLE;
        }
    };
    let if_any = matches!(parsed.command, Command::Inbox { if_any: true, .. });
    let request = match parsed.command.into_request() {
        Ok(request) => request,
        Err(message) => {
            eprintln!("erro: {message}");
            return exit::USAGE;
        }
    };
    let op = request.op();
    let response = match client.call(&request).await {
        Ok(response) => response,
        Err(e) => {
            eprintln!("erro: {e}");
            return exit::UNAVAILABLE;
        }
    };
    if !response.ok {
        return fail(&response, parsed.json);
    }
    let data = response.data.unwrap_or(serde_json::Value::Null);
    if parsed.json {
        println!("{data}");
        return exit::OK;
    }
    let text = render::render(op, &data, if_any);
    if !text.is_empty() {
        print!("{text}");
    }
    exit::OK
}

/// Erro do barramento: mensagem e dica em stderr (ou o JSON inteiro com `--json`), e o
/// exit code do contrato.
fn fail(response: &Response, json: bool) -> u8 {
    if json {
        if let Ok(line) = serde_json::to_string(response) {
            println!("{line}");
        }
    } else {
        eprintln!(
            "erro: {}",
            response.message.as_deref().unwrap_or("falha no barramento")
        );
        if let Some(hint) = &response.hint {
            eprintln!("dica: {hint}");
        }
    }
    match response.error.as_deref() {
        Some("timeout") => exit::TIMEOUT,
        Some("agent_stopped" | "unknown_agent" | "unauthorized") => exit::UNAVAILABLE,
        _ => exit::USAGE,
    }
}

impl Command {
    fn into_request(self) -> Result<Request, String> {
        Ok(match self {
            Command::Whoami => Request::Whoami,
            Command::Agents => Request::Agents,
            Command::Send { to, body } => Request::Send {
                to,
                body,
                subject: None,
                meta: Default::default(),
            },
            Command::Inbox { drain, .. } => Request::Inbox { drain },
            Command::Wait { timeout_s } => Request::Wait { timeout_s },
            Command::Note { body } => Request::Note { body },
            Command::Status { state, note } => Request::Status { state, note },
            Command::Ask {
                to,
                body,
                timeout_s,
            } => Request::Ask {
                to,
                body,
                timeout_s,
            },
            Command::Reply { reply_to, body } => Request::Reply { reply_to, body },
            Command::Notes(op) => Request::Notes(op),
            Command::Broadcast { body } => Request::Send {
                to: vec!["@all".into()],
                body,
                subject: None,
                meta: Default::default(),
            },
            Command::Help | Command::Version => {
                return Err("internal: help/version are local".into())
            }
        })
    }
}
