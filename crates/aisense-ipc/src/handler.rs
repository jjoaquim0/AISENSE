//! Tradução frame → `BusService`. Fina de propósito: validar, rotear e marcar lida é do
//! core; aqui só se escolhe a operação e se formata a resposta.

use std::path::Path;
use std::time::Duration;

use aisense_core::bus::{BusError, BusService, BusStore, Identity, Sender};
use aisense_core::notes::{NoteError, NoteSave, TeamNotes};
use serde_json::json;

use crate::protocol::{codes, NotesOp, Request, Response};
use crate::transport::Handler;

/// Timeout padrão de `wait` quando o cliente não diz (`docs/07`).
pub const WAIT_DEFAULT: Duration = Duration::from_secs(300);

pub struct BusHandler<S> {
    bus: BusService<S>,
}

impl<S> BusHandler<S> {
    pub fn new(bus: BusService<S>) -> Self {
        Self { bus }
    }

    pub fn bus(&self) -> &BusService<S> {
        &self.bus
    }
}

pub fn bus_error(error: &BusError) -> Response {
    Response::error(error.code(), error.to_string(), error.hint())
}

fn note_error(error: &NoteError) -> Response {
    Response::error(error.code(), error.to_string(), error.hint())
}

impl<S: BusStore + 'static> Handler for BusHandler<S> {
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

impl<S: BusStore + 'static> BusHandler<S> {
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
            Request::Ask { .. } | Request::Reply { .. } => Response::error(
                codes::INVALID_REQUEST,
                "ask/reply are not available yet",
                None,
            ),
            Request::Notes(op) => self.notes(me, op).await?,
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
