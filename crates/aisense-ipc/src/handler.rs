//! Tradução frame → `BusService`. Fina de propósito: validar, rotear e marcar lida é do
//! core; aqui só se escolhe a operação e se formata a resposta.

use std::path::Path;
use std::time::Duration;

use std::sync::Arc;

use aisense_core::board::{
    render_board, render_card, render_cards, render_comment, render_moved, render_next, Actor,
    BoardError, BoardService, BoardStore, NoBoardObserver, NoGate,
};
use aisense_core::bus::{BusError, BusService, Identity, Sender};
use aisense_core::notes::{NoteError, NoteSave, TeamNotes};
use aisense_core::now_ms;
use aisense_core::proposal::{
    NoProposalObserver, ProposalError, ProposalRepository, ProposalService,
};
use serde_json::json;

use crate::protocol::{codes, NotesOp, Request, Response, TaskOp};
use crate::transport::Handler;

/// Timeout padrão de `wait` quando o cliente não diz (`docs/07`).
pub const WAIT_DEFAULT: Duration = Duration::from_secs(300);

/// Timeout padrão de `task watch` (plantão).
pub const WATCH_DEFAULT: Duration = Duration::from_secs(600);

pub struct BusHandler<S> {
    bus: BusService<S>,
    board: BoardService<S>,
    proposals: ProposalService<S>,
}

impl<S: BoardStore + ProposalRepository> BusHandler<S> {
    /// O quadro traz o barramento dentro: é por ele que avisa (F06-07).
    pub fn new(board: BoardService<S>) -> Self {
        let proposals = ProposalService::new(board.bus().clone(), Arc::new(NoProposalObserver));
        Self {
            bus: board.bus().clone(),
            board,
            proposals,
        }
    }

    /// Com o serviço de propostas do app (que avisa a UI).
    pub fn with_proposals(self, proposals: ProposalService<S>) -> Self {
        Self { proposals, ..self }
    }

    /// Sem observador do quadro nem executor de gate (testes, ferramentas).
    pub fn from_bus(bus: BusService<S>) -> Self {
        Self::new(BoardService::new(
            bus,
            Arc::new(NoBoardObserver),
            Arc::new(NoGate),
        ))
    }

    pub fn bus(&self) -> &BusService<S> {
        &self.bus
    }

    pub fn board(&self) -> &BoardService<S> {
        &self.board
    }
}

pub fn proposal_error(error: &ProposalError) -> Response {
    Response::error(error.code(), error.to_string(), error.hint())
}

pub fn board_error(error: &BoardError) -> Response {
    Response::error(error.code(), error.to_string(), error.hint())
}

pub fn bus_error(error: &BusError) -> Response {
    Response::error(error.code(), error.to_string(), error.hint())
}

fn note_error(error: &NoteError) -> Response {
    Response::error(error.code(), error.to_string(), error.hint())
}

impl<S: BoardStore + ProposalRepository + 'static> Handler for BusHandler<S> {
    async fn hello(&self, token: &str) -> Result<Response, Response> {
        let identity = self
            .bus
            .authenticate(token)
            .await
            .map_err(|e| bus_error(&e))?;
        let agents = self
            .bus
            .agents(&identity.team_id)
            .await
            .map_err(|e| bus_error(&e))?;
        Ok(Response::ok(
            json!({ "identity": identity, "agents": agents }),
        ))
    }

    async fn handle(&self, token: &str, request: Request) -> Response {
        // O token vale a cada operação: sessão encerrada no meio de uma conexão longa é
        // recusada na hora (I4).
        let identity = match self.bus.authenticate(token).await {
            Ok(identity) => identity,
            Err(e) => return bus_error(&e),
        };
        match self.dispatch(&identity, request).await {
            Ok(response) => response,
            Err(response) => response,
        }
    }
}

impl<S: BoardStore + ProposalRepository + 'static> BusHandler<S> {
    async fn dispatch(&self, me: &Identity, request: Request) -> Result<Response, Response> {
        let err = |e: BusError| bus_error(&e);
        let from = Sender::Agent {
            agent_id: me.agent_id.clone(),
        };
        Ok(match request {
            Request::Hello { .. } => Response::error(
                codes::INVALID_REQUEST,
                "this connection is already authenticated",
                None,
            ),
            Request::Whoami => Response::ok(me),
            Request::Send {
                to,
                body,
                subject,
                meta,
            } => {
                let sent = self
                    .bus
                    .send(&me.team_id, from, &to, &body, subject, meta)
                    .await
                    .map_err(err)?;
                let ids: Vec<_> = sent.iter().map(|r| r.message.id.clone()).collect();
                let stopped: Vec<String> = sent
                    .iter()
                    .flat_map(|r| r.stopped.iter().map(|h| h.as_str().to_owned()))
                    .collect();
                Response::ok(json!({ "ids": ids, "stopped": stopped }))
            }
            Request::Inbox { drain } => {
                let items = self.bus.inbox(&me.agent_id, drain).await.map_err(err)?;
                let dir = self.bus.directory(&me.team_id).await.map_err(err)?;
                let views: Vec<_> = items.iter().map(|i| dir.view(&i.message)).collect();
                Response::ok(views)
            }
            Request::Wait { timeout_s } => {
                let timeout = timeout_s.map_or(WAIT_DEFAULT, |s| Duration::from_secs(s.into()));
                match self.bus.wait(&me.agent_id, timeout).await.map_err(err)? {
                    Some(item) => {
                        let dir = self.bus.directory(&me.team_id).await.map_err(err)?;
                        Response::ok(dir.view(&item.message))
                    }
                    None => Response::error(
                        "timeout",
                        format!("no message in {} s", timeout.as_secs()),
                        None,
                    ),
                }
            }
            Request::Agents => Response::ok(self.bus.agents(&me.team_id).await.map_err(err)?),
            Request::Status { state, note } => {
                let routed = self
                    .bus
                    .status(me, &state, note.as_deref())
                    .await
                    .map_err(err)?;
                Response::ok(json!({ "id": routed.message.id }))
            }
            Request::Note { body } => {
                let routed = self.bus.note(me, &body).await.map_err(err)?;
                Response::ok(json!({ "id": routed.message.id }))
            }
            Request::Ask {
                to,
                body,
                timeout_s,
            } => {
                let timeout = timeout_s.map(|s| Duration::from_secs(s.into()));
                let answer = self.bus.ask(me, &to, &body, timeout).await.map_err(err)?;
                let dir = self.bus.directory(&me.team_id).await.map_err(err)?;
                Response::ok(dir.view(&answer))
            }
            Request::Reply { reply_to, body } => {
                let routed = self.bus.reply(me, &reply_to, &body).await.map_err(err)?;
                Response::ok(json!({ "id": routed.message.id }))
            }
            Request::Notes(op) => self.notes(me, op).await?,
            Request::Board { column, full } => {
                let view = self
                    .board
                    .board(&me.team_id)
                    .await
                    .map_err(|e| board_error(&e))?;
                if let Some(slug) = &column {
                    aisense_core::board::column_by_slug(&view.columns, slug)
                        .map_err(|e| board_error(&e))?;
                }
                let text = render_board(&view, column.as_deref(), full);
                Response::ok(json!({ "text": text, "board": view }))
            }
            Request::Task(op) => self.task(me, op).await.map_err(|e| board_error(&e))?,
            Request::Propose { action, reason } => {
                let proposal = self
                    .proposals
                    .propose(me, action.clone(), &reason)
                    .await
                    .map_err(|e| proposal_error(&e))?;
                // A proposta foi registrada; a ação em si é recusada até o humano decidir.
                let described = action.describe();
                let mut chars = described.chars();
                let refused = ProposalError::NeedsApproval {
                    id: proposal.id.clone(),
                    action: chars
                        .next()
                        .map(|c| c.to_uppercase().chain(chars).collect())
                        .unwrap_or_default(),
                };
                Response {
                    data: serde_json::to_value(&proposal).ok(),
                    ..proposal_error(&refused)
                }
            }
            Request::Channels => Response::ok(self.bus.channels(&me.team_id).await.map_err(err)?),
            Request::Subscribe { channel, join } => Response::ok(
                self.bus
                    .subscribe_channel(me, &channel, join)
                    .await
                    .map_err(err)?,
            ),
        })
    }

    /// Cartões: o mesmo `BoardService` da UI. A resposta leva o texto pronto (`text`), para
    /// CLI e MCP mostrarem exatamente o mesmo, e a estrutura para quem pede `--json`.
    async fn task(&self, me: &Identity, op: TaskOp) -> Result<Response, BoardError> {
        let board = &self.board;
        let team = &me.team_id;
        let actor = Actor::Agent {
            agent_id: me.agent_id.clone(),
        };
        let moved = |m: aisense_core::board::Moved| {
            Response::ok(
                json!({ "text": render_moved(&m), "card": m.card, "warnings": m.warnings }),
            )
        };
        Ok(match op {
            TaskOp::Next => {
                let card = board.next(team, &me.agent_id).await?;
                Response::ok(json!({ "text": render_next(card.as_ref(), now_ms()), "card": card }))
            }
            TaskOp::List { filter } => {
                let cards = board.list(team, Some(&me.agent_id), &filter).await?;
                Response::ok(json!({ "text": render_cards(&cards, now_ms()), "cards": cards }))
            }
            TaskOp::Show { id } => {
                let detail = board.show(team, &id).await?;
                Response::ok(json!({ "text": render_card(&detail, now_ms()), "card": detail }))
            }
            TaskOp::Add { card } => moved(board.add(team, &actor, card).await?),
            TaskOp::Claim { id } => {
                let m = board.claim(team, &me.agent_id, &id).await?;
                let mut text = render_moved(&m);
                text.push_str(&format!(
                    "Próximo passo: aisense task move {} doing\n",
                    aisense_core::board::short_id(m.card.card.id.as_str())
                ));
                Response::ok(json!({ "text": text, "card": m.card, "warnings": m.warnings }))
            }
            TaskOp::Move { id, column, reason } => moved(
                board
                    .move_card(team, &actor, &id, &column, reason.as_deref())
                    .await?,
            ),
            TaskOp::Update { id, patch } => moved(board.update(team, &actor, &id, patch).await?),
            TaskOp::Check { id, item, undo } => moved(
                board
                    .check(team, &actor, &id, usize::try_from(item).unwrap_or(0), !undo)
                    .await?,
            ),
            TaskOp::Comment { id, body } => {
                let comment = board.comment(team, &actor, &id, &body).await?;
                Response::ok(json!({ "text": render_comment(&comment), "comment": comment }))
            }
            TaskOp::Link { id, kind, target } => {
                moved(board.link(team, &actor, &id, kind, &target).await?)
            }
            TaskOp::Block { id, reason } => moved(board.block(team, &actor, &id, &reason).await?),
            TaskOp::Done { id, note } => {
                moved(board.done(team, &actor, &id, note.as_deref()).await?)
            }
            TaskOp::Split { id, titles } => {
                let cards = board.split(team, &actor, &id, &titles).await?;
                Response::ok(json!({ "text": render_cards(&cards, now_ms()), "cards": cards }))
            }
            TaskOp::Watch { timeout_s } => {
                let timeout = timeout_s
                    .map_or(WATCH_DEFAULT, |s| Duration::from_secs(s.into()))
                    .min(aisense_core::bus::WAIT_MAX);
                match board.watch(&me.agent_id, timeout).await {
                    Some(event) => {
                        let text = match &event.card_id {
                            Some(card) => format!(
                                "{} mudou ({}). Veja com: aisense task show {}\n",
                                aisense_core::board::short_id(card.as_str()),
                                event.action,
                                aisense_core::board::short_id(card.as_str())
                            ),
                            None => format!("o quadro mudou ({})\n", event.action),
                        };
                        Response::ok(json!({ "text": text, "event": event }))
                    }
                    None => Response::error(
                        "timeout",
                        format!("nada mudou nos seus cartões em {} s", timeout.as_secs()),
                        None,
                    ),
                }
            }
            TaskOp::Approve { id, note } => {
                moved(board.approve(team, &actor, &id, note.as_deref()).await?)
            }
            TaskOp::Reject { id, reason } => moved(board.reject(team, &actor, &id, &reason).await?),
            TaskOp::Archive { id } => moved(board.archive(team, &actor, &id).await?),
        })
    }

    /// Notas da equipe: o mesmo core que a UI usa (F04-09), no diretório da equipe.
    async fn notes(&self, me: &Identity, op: NotesOp) -> Result<Response, Response> {
        let team = self
            .bus
            .store()
            .get_team(&me.team_id)
            .await
            .map_err(|e| bus_error(&e.into()))?
            .ok_or_else(|| Response::error("unauthorized", "team not found", None))?;
        let notes = TeamNotes::new(Path::new(&team.workdir));
        // Disco: fora das threads do runtime.
        let result = tokio::task::spawn_blocking(move || -> Result<Response, NoteError> {
            Ok(match op {
                NotesOp::List => Response::ok(notes.list()?),
                NotesOp::Read {
                    slug,
                    section: Some(section),
                } => Response::ok(
                    json!({ "slug": slug, "content": notes.read_section(&slug, &section)? }),
                ),
                NotesOp::Read {
                    slug,
                    section: None,
                } => Response::ok(notes.read(&slug)?),
                NotesOp::Append { slug, text } => Response::ok(notes.append(&slug, &text)?),
                NotesOp::Write {
                    slug,
                    content,
                    expect_hash,
                } => match notes.save(&slug, &content, expect_hash.as_deref())? {
                    NoteSave::Saved { note } => Response::ok(note),
                    NoteSave::Stale { current_hash, diff } => Response {
                        data: Some(json!({ "currentHash": current_hash, "diff": diff })),
                        ..Response::error(
                            "stale_note",
                            format!("note {slug:?} changed since you read it"),
                            NoteError::Stale {
                                slug: slug.clone(),
                                current_hash: String::new(),
                                diff: Vec::new(),
                            }
                            .hint(),
                        )
                    },
                },
                NotesOp::Search { query } => Response::ok(notes.search(&query)?),
                NotesOp::New { slug, title } => Response::ok(notes.create(&slug, &title)?),
            })
        })
        .await;
        match result {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(e)) => Err(note_error(&e)),
            Err(e) => Err(Response::error(codes::INTERNAL, e.to_string(), None)),
        }
    }
}
