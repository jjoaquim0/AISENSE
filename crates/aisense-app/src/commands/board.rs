//! Quadro no app (Fase 06): um `BoardService` só, o mesmo que o socket usa — a UI arrasta
//! um cartão pelas mesmas regras da CLI. Avisa a UI (`board:changed`), roda o gate de
//! revisão na bancada do responsável e dispara `card_stale` a cada minuto.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aisense_core::agent::WorkspaceMode;
use aisense_core::bench::{bench_path, effective_mode};
use aisense_core::board::{
    automations_from_toml, automations_to_toml, Activity, Actor, Automation, Board, BoardError,
    BoardEvent, BoardObserver, BoardService, BoardView, CardDetail, CardPatch, Column, ColumnDraft,
    Comment, Gate, GateFailure, GateFuture, LinkKind, Moved, NewCard,
};
use aisense_core::project::run::{resolve, run_named};
use aisense_core::project::{load_project, ProjectLookup};
use aisense_core::repo::{AgentRepository, TeamRepository};
use aisense_core::{now_ms, AgentId, CommandError, Millis, TeamId};
use aisense_store::Store;
use tauri::{AppHandle, Emitter, State};

use super::bus::Bus;

pub type BoardState = BoardService<Store>;

/// Payload: `BoardEvent`. A UI relê o quadro e realça o cartão.
pub const BOARD_CHANGED: &str = "board:changed";

/// De quanto em quanto tempo `card_stale` é conferido.
const STALE_EVERY: Duration = Duration::from_secs(60);

struct TauriBoardObserver {
    app: AppHandle,
}

impl BoardObserver for TauriBoardObserver {
    fn changed(&self, event: &BoardEvent) {
        if let Err(error) = self.app.emit(BOARD_CHANGED, event) {
            tracing::warn!(%error, "falha ao avisar a UI sobre o quadro");
        }
    }
}

/// Gate de revisão: roda os comandos nomeados do `aisense.toml` na bancada do responsável
/// (ou no diretório da equipe), como `aisense run` faria no terminal dele.
struct ProjectGate {
    store: Store,
    benches: PathBuf,
}

/// Saída de um comando do gate: só o fim fica no relatório; o eco é descartado.
struct Discard;
impl std::io::Write for Discard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn failure(command: &str, output: impl Into<String>) -> GateFailure {
    GateFailure {
        command: command.to_owned(),
        exit: None,
        output: output.into(),
    }
}

impl ProjectGate {
    async fn workdir(&self, team_id: &TeamId, assignee: Option<&AgentId>) -> Option<PathBuf> {
        let team = self.store.get_team(team_id).await.ok()??;
        let agent = match assignee {
            Some(id) => self.store.get_agent(id).await.ok()?,
            None => None,
        };
        if let Some(agent) = agent {
            if effective_mode(&team, &agent) == WorkspaceMode::PerAgent {
                let bench = bench_path(&self.benches, &team, &agent);
                if bench.is_dir() {
                    return Some(bench);
                }
            }
        }
        Some(PathBuf::from(team.workdir))
    }
}

fn run_all(dir: &Path, commands: &[String]) -> Result<(), GateFailure> {
    let config = match load_project(dir) {
        ProjectLookup::Found { config, .. } => config,
        ProjectLookup::Invalid { problem } => {
            let first = commands.first().map_or("", String::as_str);
            return Err(failure(
                first,
                format!("aisense.toml inválido: {}", problem.message),
            ));
        }
        ProjectLookup::Missing { .. } => {
            let first = commands.first().map_or("", String::as_str);
            return Err(failure(
                first,
                format!("não há aisense.toml em {}", dir.display()),
            ));
        }
    };
    for name in commands {
        let command = resolve(Some(&config), name).map_err(|e| failure(name, e.to_string()))?;
        let echo: Arc<Mutex<dyn std::io::Write + Send>> = Arc::new(Mutex::new(Discard));
        let report =
            run_named(dir, name, command, echo).map_err(|e| failure(name, e.to_string()))?;
        if !report.ok() {
            let mut output = report.tail;
            if report.timed_out {
                output.push_str(&format!("\n(parado depois de {} s)", command.timeout_s));
            }
            return Err(GateFailure {
                command: name.clone(),
                exit: report.exit_code,
                output,
            });
        }
    }
    Ok(())
}

impl Gate for ProjectGate {
    fn run<'a>(
        &'a self,
        team_id: &'a TeamId,
        assignee: Option<&'a AgentId>,
        commands: &'a [String],
    ) -> GateFuture<'a> {
        Box::pin(async move {
            if commands.is_empty() {
                return Ok(());
            }
            let first = commands[0].clone();
            let dir = self
                .workdir(team_id, assignee)
                .await
                .ok_or_else(|| failure(&first, "equipe não encontrada"))?;
            let commands = commands.to_vec();
            // Processo e disco: fora das threads do runtime.
            tokio::task::spawn_blocking(move || run_all(&dir, &commands))
                .await
                .map_err(|e| failure(&first, e.to_string()))?
        })
    }
}

pub fn setup(app: &AppHandle, bus: &Bus, store: &Store, benches: PathBuf) -> BoardState {
    let board = BoardService::new(
        bus.clone(),
        Arc::new(TauriBoardObserver { app: app.clone() }),
        Arc::new(ProjectGate {
            store: store.clone(),
            benches,
        }),
    );
    let ticker = board.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(STALE_EVERY).await;
            if let Err(error) = ticker.tick(now_ms()).await {
                tracing::warn!(%error, "card_stale não conferido");
            }
        }
    });
    board
}

fn board_error(error: BoardError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), error.hint())
}

const HUMAN: Actor = Actor::Human;

#[tauri::command]
pub async fn board_get(
    board: State<'_, BoardState>,
    team_id: TeamId,
) -> Result<BoardView, CommandError> {
    board.board(&team_id).await.map_err(board_error)
}

/// "N cartões mudaram desde que você saiu".
#[tauri::command]
pub async fn board_changes(
    board: State<'_, BoardState>,
    team_id: TeamId,
    since: Millis,
) -> Result<Vec<Activity>, CommandError> {
    board
        .changes_since(&team_id, since)
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn card_show(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
) -> Result<CardDetail, CommandError> {
    board.show(&team_id, &id).await.map_err(board_error)
}

#[tauri::command]
pub async fn card_add(
    board: State<'_, BoardState>,
    team_id: TeamId,
    card: NewCard,
) -> Result<Moved, CommandError> {
    board.add(&team_id, &HUMAN, card).await.map_err(board_error)
}

/// Arrastar entre colunas: as mesmas regras de `aisense task move` (WIP, motivo, gate).
#[tauri::command]
pub async fn card_move(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
    column: String,
    reason: Option<String>,
) -> Result<Moved, CommandError> {
    board
        .move_card(&team_id, &HUMAN, &id, &column, reason.as_deref())
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn card_update(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
    patch: CardPatch,
) -> Result<Moved, CommandError> {
    board
        .update(&team_id, &HUMAN, &id, patch)
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn card_check(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
    item: usize,
    done: bool,
) -> Result<Moved, CommandError> {
    board
        .check(&team_id, &HUMAN, &id, item, done)
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn card_comment(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
    body: String,
) -> Result<Comment, CommandError> {
    board
        .comment(&team_id, &HUMAN, &id, &body)
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn card_link(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
    kind: LinkKind,
    target: String,
) -> Result<Moved, CommandError> {
    board
        .link(&team_id, &HUMAN, &id, kind, &target)
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn card_approve(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
    note: Option<String>,
) -> Result<Moved, CommandError> {
    board
        .approve(&team_id, &HUMAN, &id, note.as_deref())
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn card_reject(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
    reason: String,
) -> Result<Moved, CommandError> {
    board
        .reject(&team_id, &HUMAN, &id, &reason)
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn card_archive(
    board: State<'_, BoardState>,
    team_id: TeamId,
    id: String,
) -> Result<Moved, CommandError> {
    board
        .archive(&team_id, &HUMAN, &id)
        .await
        .map_err(board_error)
}

/// Editor de colunas (F06-10). `moves`: `[slug removido, slug destino]`.
#[tauri::command]
pub async fn board_columns_save(
    board: State<'_, BoardState>,
    team_id: TeamId,
    columns: Vec<ColumnDraft>,
    moves: Vec<(String, String)>,
) -> Result<Vec<Column>, CommandError> {
    board
        .save_columns(&team_id, columns, moves)
        .await
        .map_err(board_error)
}

#[tauri::command]
pub async fn board_automations_save(
    board: State<'_, BoardState>,
    team_id: TeamId,
    automations: Vec<Automation>,
) -> Result<Board, CommandError> {
    board
        .set_automations(&team_id, automations)
        .await
        .map_err(board_error)
}

/// O TOML que o editor mostra ao lado do formulário.
#[tauri::command]
pub fn board_automations_toml(automations: Vec<Automation>) -> String {
    automations_to_toml(&automations)
}

#[tauri::command]
pub fn board_automations_parse(source: String) -> Result<Vec<Automation>, CommandError> {
    automations_from_toml(&source)
        .map_err(|message| CommandError::new("invalid_automation", message, None))
}
