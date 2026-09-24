//! Linha de comando à mão: poucos comandos, sem dependência de parser (a CLI precisa subir
//! rápido e ser pequena). `--json` vale em qualquer posição.

use crate::protocol::{NotesOp, Request};

pub const HELP: &str = "\
aisense — fale com a sua equipe de agentes (AISENSE)

  aisense agents                         quem está na equipe, papel e estado
  aisense send @alguem \"texto\"           avisa; não espera resposta (#canal, @all, @voce)
  aisense broadcast \"texto\"              avisa a equipe inteira
  aisense ask @alguem \"pergunta\"         pergunta e espera a resposta [--timeout 300]
  aisense reply <id> \"texto\"             responde uma pergunta que te fizeram
  aisense inbox [--drain] [--if-any]     mensagens não lidas (--drain marca como lidas)
                 [--hook-json]           formato do hook Stop do Claude Code
  aisense wait [--timeout 120]           espera a próxima mensagem
  aisense note \"texto\"                   registra na linha do tempo
  aisense status \"estado\" [--note x]     diz o que você está fazendo
  aisense whoami                         seu endereço e equipe
  aisense notes list|read|append|write|search|new ...   notas da equipe

  --json em qualquer comando devolve JSON.
  Saída: 0 ok · 1 erro de uso · 2 timeout · 3 destinatário ou AISENSE indisponível.
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Whoami,
    Agents,
    Send {
        to: Vec<String>,
        body: String,
    },
    Broadcast {
        body: String,
    },
    Ask {
        to: String,
        body: String,
        timeout_s: Option<u32>,
    },
    Reply {
        reply_to: String,
        body: String,
    },
    Inbox {
        drain: bool,
        if_any: bool,
        /// Saída no formato do hook `Stop` do Claude Code (F05-08).
        hook_json: bool,
    },
    Wait {
        timeout_s: Option<u32>,
    },
    Note {
        body: String,
    },
    Status {
        state: String,
        note: Option<String>,
    },
    Notes(NotesOp),
}

impl Command {
    /// O frame que este comando manda. `Help`/`Version` são locais: `None`.
    pub fn request(&self) -> Option<Request> {
        Some(match self.clone() {
            Command::Whoami => Request::Whoami,
            Command::Agents => Request::Agents,
            Command::Send { to, body } => Request::Send {
                to,
                body,
                subject: None,
                meta: Default::default(),
            },
            Command::Broadcast { body } => Request::Send {
                to: vec!["@all".into()],
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
            Command::Help | Command::Version => return None,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    pub command: Command,
    pub json: bool,
}

pub fn parse(argv: &[String]) -> Result<Parsed, String> {
    let json = argv.iter().any(|a| a == "--json");
    let mut args: Vec<String> = argv.iter().filter(|a| *a != "--json").cloned().collect();
    if args.is_empty() {
        return Ok(Parsed {
            command: Command::Help,
            json,
        });
    }
    let name = args.remove(0);
    let command = match name.as_str() {
        "-h" | "--help" | "help" => Command::Help,
        "-V" | "--version" => Command::Version,
        "whoami" => no_args(&args, Command::Whoami)?,
        "agents" => no_args(&args, Command::Agents)?,
        "send" => {
            let (to, rest): (Vec<_>, Vec<_>) = args
                .into_iter()
                .partition(|a| a.starts_with('@') || a.starts_with('#'));
            if to.is_empty() {
                return Err("diga para quem: aisense send @alguem \"texto\"".into());
            }
            Command::Send {
                to,
                body: body(rest)?,
            }
        }
        "broadcast" => Command::Broadcast { body: body(args)? },
        "ask" => {
            let timeout_s = take_number(&mut args, "--timeout")?;
            if args.is_empty() || !args[0].starts_with('@') {
                return Err("diga para quem: aisense ask @alguem \"pergunta\"".into());
            }
            let to = args.remove(0);
            Command::Ask {
                to,
                body: body(args)?,
                timeout_s,
            }
        }
        "reply" => {
            if args.is_empty() {
                return Err("diga qual pergunta: aisense reply <id> \"texto\"".into());
            }
            let reply_to = args.remove(0);
            Command::Reply {
                reply_to,
                body: body(args)?,
            }
        }
        "inbox" => {
            let drain = take_flag(&mut args, "--drain");
            let if_any = take_flag(&mut args, "--if-any");
            let hook_json = take_flag(&mut args, "--hook-json");
            no_args(
                &args,
                Command::Inbox {
                    drain,
                    if_any,
                    hook_json,
                },
            )?
        }
        "wait" => {
            let timeout_s = take_number(&mut args, "--timeout")?;
            no_args(&args, Command::Wait { timeout_s })?
        }
        "note" => Command::Note { body: body(args)? },
        "status" => {
            let note = take_value(&mut args, "--note")?;
            Command::Status {
                state: body(args)?,
                note,
            }
        }
        "notes" => Command::Notes(notes(args)?),
        other => return Err(format!("comando desconhecido: {other}")),
    };
    Ok(Parsed { command, json })
}

fn no_args(args: &[String], command: Command) -> Result<Command, String> {
    match args.first() {
        None => Ok(command),
        Some(extra) => Err(format!("argumento inesperado: {extra}")),
    }
}

/// O texto: o resto dos argumentos juntos (quem esquece as aspas também é atendido).
fn body(args: Vec<String>) -> Result<String, String> {
    let text = args.join(" ");
    if text.trim().is_empty() {
        return Err("faltou o texto (entre aspas)".into());
    }
    Ok(text)
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    let before = args.len();
    args.retain(|a| a != flag);
    args.len() != before
}

fn take_value(args: &mut Vec<String>, flag: &str) -> Result<Option<String>, String> {
    let Some(at) = args.iter().position(|a| a == flag) else {
        return Ok(None);
    };
    if at + 1 >= args.len() {
        return Err(format!("{flag} precisa de um valor"));
    }
    let value = args.remove(at + 1);
    args.remove(at);
    Ok(Some(value))
}

fn take_number(args: &mut Vec<String>, flag: &str) -> Result<Option<u32>, String> {
    take_value(args, flag)?
        .map(|v| {
            v.parse::<u32>()
                .map_err(|_| format!("{flag} espera segundos, recebeu {v:?}"))
        })
        .transpose()
}

fn notes(mut args: Vec<String>) -> Result<NotesOp, String> {
    if args.is_empty() {
        return Err("aisense notes list|read|append|write|search|new".into());
    }
    let action = args.remove(0);
    let slug = |args: &mut Vec<String>| -> Result<String, String> {
        if args.is_empty() {
            Err("faltou o nome da nota".into())
        } else {
            Ok(args.remove(0))
        }
    };
    Ok(match action.as_str() {
        "list" => {
            no_args(&args, Command::Help)?;
            NotesOp::List
        }
        "read" => {
            let section = take_value(&mut args, "--section")?;
            let slug = slug(&mut args)?;
            no_args(&args, Command::Help)?;
            NotesOp::Read { slug, section }
        }
        "append" => {
            let slug = slug(&mut args)?;
            NotesOp::Append {
                slug,
                text: body(args)?,
            }
        }
        "write" => {
            let file = take_value(&mut args, "--file")?;
            let expect_hash = take_value(&mut args, "--expect-hash")?;
            let slug = slug(&mut args)?;
            let content = match file {
                Some(path) => std::fs::read_to_string(&path)
                    .map_err(|e| format!("não consegui ler {path}: {e}"))?,
                None => body(args)?,
            };
            NotesOp::Write {
                slug,
                content,
                expect_hash,
            }
        }
        "search" => NotesOp::Search { query: body(args)? },
        "new" => {
            let title = take_value(&mut args, "--title")?.unwrap_or_default();
            let slug = slug(&mut args)?;
            no_args(&args, Command::Help)?;
            NotesOp::New { slug, title }
        }
        other => return Err(format!("ação desconhecida em notes: {other}")),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn p(line: &[&str]) -> Result<Parsed, String> {
        parse(&line.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn comandos_da_skill_trabalho_em_equipe() {
        assert_eq!(
            p(&["send", "@frontend", "contrato", "subiu"])
                .unwrap()
                .command,
            Command::Send {
                to: vec!["@frontend".into()],
                body: "contrato subiu".into()
            }
        );
        let parsed = p(&["inbox", "--drain", "--json"]).unwrap();
        assert!(parsed.json);
        assert_eq!(
            parsed.command,
            Command::Inbox {
                drain: true,
                if_any: false,
                hook_json: false
            }
        );
        assert_eq!(
            p(&["ask", "@revisor", "revisa?", "--timeout", "60"])
                .unwrap()
                .command,
            Command::Ask {
                to: "@revisor".into(),
                body: "revisa?".into(),
                timeout_s: Some(60)
            }
        );
        assert_eq!(
            p(&["notes", "read", "api", "--section", "Auth"])
                .unwrap()
                .command,
            Command::Notes(NotesOp::Read {
                slug: "api".into(),
                section: Some("Auth".into())
            })
        );
        assert_eq!(p(&[]).unwrap().command, Command::Help);
    }

    #[test]
    fn erros_de_uso_explicam_o_que_falta() {
        assert!(p(&["send", "oi"]).unwrap_err().contains("para quem"));
        assert!(p(&["send", "@x"]).unwrap_err().contains("texto"));
        assert!(p(&["wait", "--timeout", "dois"])
            .unwrap_err()
            .contains("segundos"));
        assert!(p(&["voar"]).unwrap_err().contains("desconhecido"));
        assert!(p(&["agents", "demais"]).is_err());
    }
}
