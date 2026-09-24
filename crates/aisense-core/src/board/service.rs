//! As operações do quadro como as fachadas as veem (`docs/13`, "API dos agentes"). CLI e
//! MCP chegam pelo socket, a UI por comando Tauri; todas passam por aqui.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast;
use ts_rs::TS;

use super::automation::{self, Action, Automation, Trigger};
use super::model::{
    normalize_label, Activity, Actor, Board, Card, CardLink, CardPriority, ChecklistItem, Column,
    ColumnKind, Comment, LinkKind, BODY_MAX, TITLE_MAX,
};
use super::repo::{BoardRepository, CardQuery, CardWrite, WriteGuard};
use super::rules::{
    check_dependency, check_transition, check_wip, column_by_slug, default_columns, first_of_kind,
    is_open, valid_slug, Wip,
};
use super::{BoardError, BoardResult};
use crate::agent::{Agent, Handle};
use crate::bus::{Address, BusService, BusStore, MessageKind, Outgoing, Sender};
use crate::color::AgentColor;
use crate::ids::{ActivityId, AgentId, BoardId, CardId, ColumnId, CommentId, TeamId};
use crate::repo::{AgentRepository, RepoError, TeamFilter};
use crate::time::{now_ms, Millis};

/// Tudo que o serviço precisa do banco.
pub trait BoardStore: BoardRepository + BusStore {}
impl<T: BoardRepository + BusStore> BoardStore for T {}

/// Quantas vezes uma automação pode disparar outra antes de ser cortada (`docs/13`, riscos:
/// "A move para X, que move para Y, que move para X").
pub const AUTOMATION_MAX_DEPTH: u32 = 5;
/// Teto de cartões criados por agente por hora (`FASE-06`, riscos: quadro poluído).
pub const CARDS_PER_AGENT_PER_HOUR: usize = 30;
/// Tentativas de gravação quando outro escritor muda o cartão no meio (versão).
const WRITE_ATTEMPTS: u32 = 8;

/// O agente como o quadro o mostra: handle e cor (borda do cartão).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AgentTag {
    pub id: AgentId,
    pub handle: Handle,
    pub color: AgentColor,
}

pub type AgentHandles = Vec<AgentTag>;

/// Um cartão com o que a leitura precisa ao lado (handles, dependências abertas).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct CardView {
    #[serde(flatten)]
    pub card: Card,
    pub column_slug: String,
    pub assignee_handle: Option<String>,
    /// Dependências ainda abertas.
    pub blocked_by: Vec<CardId>,
    pub comments: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct BoardView {
    pub team_id: TeamId,
    pub team_name: String,
    pub board: Board,
    pub columns: Vec<Column>,
    pub cards: Vec<CardView>,
    pub agents: AgentHandles,
    pub now: Millis,
}

/// Um cartão vizinho (dependência, dependente, subtarefa).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct CardRef {
    pub id: CardId,
    pub title: String,
    pub column_slug: String,
    pub open: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct CardDetail {
    pub card: CardView,
    pub column: Column,
    pub comments: Vec<Comment>,
    pub activity: Vec<Activity>,
    pub depends_on: Vec<CardRef>,
    pub dependents: Vec<CardRef>,
    pub children: Vec<CardRef>,
    pub agents: AgentHandles,
}

/// Resultado de uma operação que muda cartão: o cartão e avisos que não impedem
/// (`blocked_by_open`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Moved {
    pub card: CardView,
    pub warnings: Vec<String>,
}

/// Algo mudou no quadro: acorda `task watch`, a UI realça o cartão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct BoardEvent {
    pub team_id: TeamId,
    pub card_id: Option<CardId>,
    pub action: String,
    pub actor: Actor,
    /// Agentes a quem a mudança diz respeito (responsável antes e depois, criador).
    pub involved: Vec<AgentId>,
    pub at: Millis,
}

pub trait BoardObserver: Send + Sync + 'static {
    fn changed(&self, _event: &BoardEvent) {}
}

pub struct NoBoardObserver;
impl BoardObserver for NoBoardObserver {}

/// Um comando do gate que falhou, com a saída para anexar ao cartão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateFailure {
    pub command: String,
    pub exit: Option<i32>,
    pub output: String,
}

pub type GateFuture<'a> = Pin<Box<dyn Future<Output = Result<(), GateFailure>> + Send + 'a>>;

/// Roda os `requires_commands` de uma coluna na bancada do responsável (`docs/13`, gate).
/// O app implementa com `project::run_named`; os testes, com um falso.
pub trait Gate: Send + Sync + 'static {
    fn run<'a>(
        &'a self,
        team_id: &'a TeamId,
        assignee: Option<&'a AgentId>,
        commands: &'a [String],
    ) -> GateFuture<'a>;
}

/// Sem executor: nenhum comando passa sem ter rodado.
pub struct NoGate;
impl Gate for NoGate {
    fn run<'a>(
        &'a self,
        _team_id: &'a TeamId,
        _assignee: Option<&'a AgentId>,
        commands: &'a [String],
    ) -> GateFuture<'a> {
        Box::pin(async move {
            match commands.first() {
                None => Ok(()),
                Some(command) => Err(GateFailure {
                    command: command.clone(),
                    exit: None,
                    output: "nenhum executor de comandos disponível aqui".into(),
                }),
            }
        })
    }
}

/// Um cartão novo (`aisense task add`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct NewCard {
    pub title: String,
    pub body: String,
    /// Slug; sem ele, a primeira coluna `ready`.
    pub column: Option<String>,
    /// `@handle` ou `handle`.
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    pub priority: Option<CardPriority>,
    pub blocked_by: Vec<String>,
    pub checklist: Vec<String>,
    pub parent: Option<String>,
    /// Motivo, se a coluna for de bloqueio.
    pub reason: Option<String>,
}

/// `aisense task update`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct CardPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    /// `Some("")` tira o responsável.
    pub assignee: Option<String>,
    pub add_labels: Vec<String>,
    pub remove_labels: Vec<String>,
    pub priority: Option<CardPriority>,
    pub add_checklist: Vec<String>,
    pub blocked_by: Vec<String>,
    pub unblock: Vec<String>,
}

/// `aisense task list`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct CardFilter {
    pub column: Option<String>,
    pub assignee: Option<String>,
    pub mine: bool,
    pub unassigned: bool,
    pub label: Option<String>,
    /// Inclui os concluídos (coluna terminal). Por padrão ficam de fora.
    pub include_done: bool,
}

/// Uma coluna no editor (F06-10). `id` presente = coluna existente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct ColumnDraft {
    pub id: Option<ColumnId>,
    pub slug: String,
    pub name: String,
    pub kind: ColumnKind,
    pub wip_limit: Option<u32>,
    pub wip_per_agent: Option<u32>,
    pub requires_approval: bool,
    pub approver_must_differ: bool,
    pub requires_commands: Vec<String>,
}

/// O que uma edição fez, para histórico e automações.
struct Change {
    action: &'static str,
    detail: serde_json::Value,
    require_unassigned: bool,
}

impl Change {
    fn new(action: &'static str, detail: serde_json::Value) -> Self {
        Self {
            action,
            detail,
            require_unassigned: false,
        }
    }
}

/// O quadro já carregado para uma operação.
struct Ctx {
    board: Board,
    columns: Vec<Column>,
    agents: Vec<Agent>,
}

impl Ctx {
    fn column(&self, id: &ColumnId) -> Option<&Column> {
        self.columns.iter().find(|c| &c.id == id)
    }

    fn slug(&self, id: &ColumnId) -> String {
        self.column(id).map(|c| c.slug.clone()).unwrap_or_default()
    }

    fn handle(&self, id: &AgentId) -> Option<String> {
        self.agents
            .iter()
            .find(|a| &a.id == id)
            .map(|a| a.handle.as_str().to_owned())
    }

    fn agent_by_handle(&self, raw: &str) -> BoardResult<AgentId> {
        let name = raw.trim().trim_start_matches('@');
        self.agents
            .iter()
            .find(|a| a.handle.as_str() == name)
            .map(|a| a.id.clone())
            .ok_or_else(|| BoardError::UnknownAgent(name.to_owned()))
    }

    fn tags(&self) -> AgentHandles {
        self.agents
            .iter()
            .map(|a| AgentTag {
                id: a.id.clone(),
                handle: a.handle.clone(),
                color: a.color,
            })
            .collect()
    }

    fn label(&self, actor: &Actor) -> String {
        match actor {
            Actor::Human => "@voce".into(),
            Actor::System => "AISENSE".into(),
            Actor::Agent { agent_id } => self
                .handle(agent_id)
                .map_or_else(|| "agente removido".into(), |h| format!("@{h}")),
        }
    }
}

/// Uma automação a avaliar depois de uma gravação.
struct Fired {
    trigger: Trigger,
    column: ColumnId,
}

pub struct BoardService<S> {
    bus: BusService<S>,
    observer: Arc<dyn BoardObserver>,
    gate: Arc<dyn Gate>,
    hub: broadcast::Sender<BoardEvent>,
    /// `card_stale` já avisado: (cartão, desde quando na coluna, índice da automação).
    stale_fired: Arc<Mutex<HashSet<(CardId, Millis, usize)>>>,
}

impl<S> Clone for BoardService<S> {
    fn clone(&self) -> Self {
        Self {
            bus: self.bus.clone(),
            observer: Arc::clone(&self.observer),
            gate: Arc::clone(&self.gate),
            hub: self.hub.clone(),
            stale_fired: Arc::clone(&self.stale_fired),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Cria o quadro da equipe com as colunas e automações padrão. Idempotente: equipe que já
/// tem quadro fica como está.
pub async fn ensure_board<S: BoardRepository + AgentRepository>(
    store: &S,
    team_id: &TeamId,
    now: Millis,
) -> Result<Board, RepoError> {
    if let Some(board) = store.get_board(team_id).await? {
        return Ok(board);
    }
    let agents = store.list_agents(team_id).await?;
    let reviewer = agents
        .iter()
        .find(|a| {
            let h = a.handle.as_str();
            h.contains("revis") || h.contains("review")
        })
        .map(|a| a.handle.as_str().to_owned());
    let id = BoardId::new();
    let columns = default_columns(&id);
    let board = Board {
        id,
        team_id: team_id.clone(),
        automations: automation::defaults(&columns, reviewer.as_deref()),
        created_at: now,
    };
    match store.create_board(&board, &columns).await {
        Ok(()) => Ok(board),
        // Outro chamador criou no meio: vale o dele.
        Err(RepoError::AlreadyExists(_)) => store
            .get_board(team_id)
            .await?
            .ok_or_else(|| RepoError::Corrupt("board vanished".into())),
        Err(e) => Err(e),
    }
}

fn check_text(title: &str, body: &str) -> BoardResult<()> {
    if title.trim().is_empty() {
        return Err(BoardError::InvalidRequest(
            "O cartão precisa de um título.".into(),
        ));
    }
    if title.chars().count() > TITLE_MAX {
        return Err(BoardError::InvalidRequest(format!(
            "Título longo demais (máximo {TITLE_MAX} caracteres): detalhe no corpo."
        )));
    }
    if body.len() > BODY_MAX {
        return Err(BoardError::InvalidRequest(format!(
            "Texto longo demais (máximo {} KiB): anexe o caminho de um arquivo.",
            BODY_MAX / 1024
        )));
    }
    Ok(())
}

fn labels_of(raw: &[String]) -> BoardResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for item in raw.iter().flat_map(|l| l.split(',')) {
        if item.trim().is_empty() {
            continue;
        }
        let label = normalize_label(item)
            .ok_or_else(|| BoardError::InvalidRequest(format!("Label inválida: '{item}'.")))?;
        if !out.contains(&label) {
            out.push(label);
        }
    }
    Ok(out)
}

fn ago(now: Millis, then: Millis) -> u64 {
    u64::try_from((now - then).max(0) / 1000).unwrap_or(0)
}

impl<S: BoardStore> BoardService<S> {
    pub fn new(bus: BusService<S>, observer: Arc<dyn BoardObserver>, gate: Arc<dyn Gate>) -> Self {
        let (hub, _) = broadcast::channel(1024);
        Self {
            bus,
            observer,
            gate,
            hub,
            stale_fired: Arc::default(),
        }
    }

    pub fn bus(&self) -> &BusService<S> {
        &self.bus
    }

    fn store(&self) -> &S {
        self.bus.store()
    }

    /// Escuta toda mudança do quadro daqui em diante.
    pub fn subscribe(&self) -> broadcast::Receiver<BoardEvent> {
        self.hub.subscribe()
    }

    async fn ctx(&self, team_id: &TeamId) -> BoardResult<Ctx> {
        let board = ensure_board(self.store(), team_id, now_ms()).await?;
        let columns = self.store().list_columns(&board.id).await?;
        let agents = self.store().list_agents(team_id).await?;
        Ok(Ctx {
            board,
            columns,
            agents,
        })
    }

    /// Acha o cartão pelo id inteiro ou por um pedaço único dele (`7K2` de `tsk_…7K2`).
    async fn find(&self, team_id: &TeamId, raw: &str) -> BoardResult<Card> {
        let raw = raw.trim();
        if let Some(card) = self.store().get_card(&CardId::from_raw(raw)).await? {
            if &card.team_id == team_id {
                return Ok(card);
            }
            return Err(BoardError::UnknownCard(raw.to_owned()));
        }
        // `tsk_ABC123` (o id curto do `aisense board`) ou só `ABC123`.
        let needle = raw
            .strip_prefix(CardId::PREFIX)
            .and_then(|r| r.strip_prefix('_'))
            .unwrap_or(raw)
            .to_uppercase();
        if needle.len() < 3 {
            return Err(BoardError::UnknownCard(raw.to_owned()));
        }
        let query = CardQuery {
            include_archived: true,
            ..CardQuery::default()
        };
        let mut hits: Vec<Card> = self
            .store()
            .list_cards(team_id, &query)
            .await?
            .into_iter()
            .filter(|c| c.id.as_str().to_uppercase().ends_with(&needle))
            .collect();
        match hits.len() {
            1 => Ok(hits.remove(0)),
            0 => Err(BoardError::UnknownCard(raw.to_owned())),
            _ => Err(BoardError::InvalidRequest(format!(
                "'{raw}' casa com {} cartões: use o id inteiro.",
                hits.len()
            ))),
        }
    }

    async fn view(&self, ctx: &Ctx, card: Card) -> BoardResult<CardView> {
        let edges = self.store().dependencies(&card.team_id).await?;
        let deps: Vec<CardId> = edges
            .iter()
            .filter(|(t, _)| t == &card.id)
            .map(|(_, d)| d.clone())
            .collect();
        let mut blocked_by = Vec::new();
        for dep in deps {
            if let Some(other) = self.store().get_card(&dep).await? {
                if is_open(&other, &ctx.columns) {
                    blocked_by.push(dep);
                }
            }
        }
        let comments =
            u32::try_from(self.store().list_comments(&card.id).await?.len()).unwrap_or(u32::MAX);
        Ok(CardView {
            column_slug: ctx.slug(&card.column_id),
            assignee_handle: card.assignee.as_ref().and_then(|a| ctx.handle(a)),
            blocked_by,
            comments,
            card,
        })
    }

    fn emit(
        &self,
        ctx: &Ctx,
        card: Option<&Card>,
        before: Option<&Card>,
        action: &str,
        actor: &Actor,
    ) {
        let mut involved: Vec<AgentId> = Vec::new();
        for c in card.into_iter().chain(before) {
            for id in c.assignee.iter().chain(c.created_by.iter()) {
                if !involved.contains(id) {
                    involved.push(id.clone());
                }
            }
        }
        let event = BoardEvent {
            team_id: ctx.board.team_id.clone(),
            card_id: card.map(|c| c.id.clone()),
            action: action.to_owned(),
            actor: actor.clone(),
            involved,
            at: now_ms(),
        };
        self.observer.changed(&event);
        let _ = self.hub.send(event);
    }

    /// Mensagem de sistema pelo barramento, que respeita o `delivery_mode` de cada agente
    /// (caixa, injeção ou hook). Falha ao avisar não desfaz a mudança do quadro.
    async fn notify(&self, team_id: &TeamId, to: Address, body: String) {
        let out = Outgoing {
            kind: MessageKind::System,
            subject: Some("quadro".into()),
            ..Outgoing::message(Sender::System, to, body)
        };
        if let Err(error) = self.bus.dispatch(team_id, out).await {
            tracing::warn!(%error, "aviso do quadro não entregue");
        }
    }

    async fn notify_agent(&self, ctx: &Ctx, agent: &AgentId, actor: &Actor, body: String) {
        if actor.agent() == Some(agent) {
            return;
        }
        if let Some(agent) = ctx.agents.iter().find(|a| &a.id == agent) {
            self.notify(
                &ctx.board.team_id,
                Address::Agent(agent.handle.clone()),
                body,
            )
            .await;
        }
    }

    async fn record(
        &self,
        card: &CardId,
        actor: &Actor,
        action: &str,
        detail: serde_json::Value,
    ) -> BoardResult<()> {
        self.store()
            .insert_activity(&Activity {
                id: ActivityId::new(),
                card_id: card.clone(),
                actor: actor.clone(),
                action: action.to_owned(),
                detail,
                created_at: now_ms(),
            })
            .await?;
        Ok(())
    }

    /// Qual limite barrou `card`, com os números de agora.
    async fn wip_error(&self, ctx: &Ctx, card: &Card) -> BoardResult<BoardError> {
        let column = ctx
            .column(&card.column_id)
            .ok_or_else(|| BoardError::InvalidRequest("coluna sumiu".into()))?;
        let cards = self
            .store()
            .list_cards(
                &card.team_id,
                &CardQuery {
                    column_id: Some(column.id.clone()),
                    ..CardQuery::default()
                },
            )
            .await?;
        Ok(
            match check_wip(column, &cards, &card.id, card.assignee.as_ref()) {
                Wip::FullForAgent { count, limit } => BoardError::WipPerAgent {
                    column: column.name.clone(),
                    who: card
                        .assignee
                        .as_ref()
                        .and_then(|a| ctx.handle(a))
                        .map_or_else(|| "o responsável".into(), |h| format!("@{h}")),
                    count,
                    limit,
                },
                Wip::Full { count, limit } => BoardError::WipExceeded {
                    column: column.name.clone(),
                    count,
                    limit,
                },
                // Liberou entre a gravação e a leitura: quem chamou tenta de novo.
                Wip::Fits => BoardError::Conflict {
                    card: card.id.clone(),
                    attempts: 1,
                },
            },
        )
    }

    /// Lê, aplica `edit` e grava com trava otimista; repete se outro escritor passou na
    /// frente. Depois grava o histórico, avisa quem precisa e roda as automações.
    async fn update_with<F>(
        &self,
        ctx: &Ctx,
        card_id: &CardId,
        actor: &Actor,
        depth: u32,
        mut edit: F,
    ) -> BoardResult<Card>
    where
        F: FnMut(&mut Card, &Ctx) -> BoardResult<Option<Change>> + Send,
    {
        for _ in 0..WRITE_ATTEMPTS {
            let before = self
                .store()
                .get_card(card_id)
                .await?
                .ok_or_else(|| BoardError::UnknownCard(card_id.to_string()))?;
            let mut after = before.clone();
            let Some(change) = edit(&mut after, ctx)? else {
                return Ok(before);
            };
            let now = now_ms();
            let moved = after.column_id != before.column_id;
            if moved {
                after.column_since = now;
                let leaving_blocked = ctx
                    .column(&before.column_id)
                    .is_some_and(|c| c.kind == ColumnKind::Blocked);
                if leaving_blocked {
                    after.block_reason = None;
                }
            }
            after.version = before.version + 1;
            after.updated_at = now;
            let destination = ctx.column(&after.column_id);
            let checks_wip = moved || after.assignee != before.assignee;
            let guard = WriteGuard {
                expected_version: before.version,
                require_unassigned: change.require_unassigned,
                wip_limit: destination.filter(|_| checks_wip).and_then(|c| c.wip_limit),
                wip_per_agent: destination
                    .filter(|_| checks_wip)
                    .and_then(|c| c.wip_per_agent),
            };
            match self.store().update_card(&after, &guard).await? {
                CardWrite::Written => {
                    self.after_write(ctx, &before, &after, actor, change, depth)
                        .await?;
                    return Ok(after);
                }
                CardWrite::WipFull => return Err(self.wip_error(ctx, &after).await?),
                CardWrite::Stale => continue,
            }
        }
        Err(BoardError::Conflict {
            card: card_id.clone(),
            attempts: WRITE_ATTEMPTS,
        })
    }

    async fn after_write(
        &self,
        ctx: &Ctx,
        before: &Card,
        after: &Card,
        actor: &Actor,
        change: Change,
        depth: u32,
    ) -> BoardResult<()> {
        self.record(&after.id, actor, change.action, change.detail)
            .await?;
        if after.assignee != before.assignee {
            if let Some(assignee) = &after.assignee {
                let body = format!(
                    "Cartão {} atribuído a você por {}: {}\nVeja com: aisense task show {}",
                    after.id,
                    ctx.label(actor),
                    after.title,
                    after.id
                );
                self.notify_agent(ctx, assignee, actor, body).await;
            }
        }
        self.emit(ctx, Some(after), Some(before), change.action, actor);
        let mut fired = Vec::new();
        if after.column_id != before.column_id {
            fired.push(Fired {
                trigger: Trigger::CardLeaves,
                column: before.column_id.clone(),
            });
            fired.push(Fired {
                trigger: Trigger::CardEnters,
                column: after.column_id.clone(),
            });
        }
        let (done, total) = after.checklist_progress();
        let (was_done, _) = before.checklist_progress();
        if total > 0 && done == total && was_done < total {
            fired.push(Fired {
                trigger: Trigger::ChecklistComplete,
                column: after.column_id.clone(),
            });
        }
        for f in fired {
            Box::pin(self.automate(ctx, &after.id, f, actor, depth)).await;
        }
        Ok(())
    }

    // ───────────────────────────── leitura ─────────────────────────────

    /// O quadro inteiro (`aisense board`, T8).
    pub async fn board(&self, team_id: &TeamId) -> BoardResult<BoardView> {
        let ctx = self.ctx(team_id).await?;
        let team = self
            .store()
            .get_team(team_id)
            .await?
            .ok_or_else(|| RepoError::TeamNotFound(team_id.clone()))?;
        let cards = self
            .store()
            .list_cards(team_id, &CardQuery::default())
            .await?;
        let views = self.views(&ctx, cards).await?;
        Ok(BoardView {
            team_id: team_id.clone(),
            team_name: team.name,
            agents: ctx.tags(),
            board: ctx.board,
            columns: ctx.columns,
            cards: views,
            now: now_ms(),
        })
    }

    /// Várias de uma vez, com três consultas no total (não uma por cartão).
    async fn views(&self, ctx: &Ctx, cards: Vec<Card>) -> BoardResult<Vec<CardView>> {
        let Some(team_id) = cards.first().map(|c| c.team_id.clone()) else {
            return Ok(Vec::new());
        };
        let edges = self.store().dependencies(&team_id).await?;
        let everything = self
            .store()
            .list_cards(
                &team_id,
                &CardQuery {
                    include_archived: true,
                    ..CardQuery::default()
                },
            )
            .await?;
        let open: BTreeSet<&CardId> = everything
            .iter()
            .filter(|c| is_open(c, &ctx.columns))
            .map(|c| &c.id)
            .collect();
        let mut blocked: BTreeMap<CardId, Vec<CardId>> = BTreeMap::new();
        for (task, dep) in &edges {
            if open.contains(dep) {
                blocked.entry(task.clone()).or_default().push(dep.clone());
            }
        }
        let counts: HashMap<CardId, u32> = self
            .store()
            .comment_counts(&team_id)
            .await?
            .into_iter()
            .collect();
        Ok(cards
            .into_iter()
            .map(|card| CardView {
                column_slug: ctx.slug(&card.column_id),
                assignee_handle: card.assignee.as_ref().and_then(|a| ctx.handle(a)),
                blocked_by: blocked.remove(&card.id).unwrap_or_default(),
                comments: counts.get(&card.id).copied().unwrap_or(0),
                card,
            })
            .collect())
    }

    pub async fn list(
        &self,
        team_id: &TeamId,
        me: Option<&AgentId>,
        filter: &CardFilter,
    ) -> BoardResult<Vec<CardView>> {
        let ctx = self.ctx(team_id).await?;
        let mut query = CardQuery {
            unassigned: filter.unassigned,
            label: filter.label.as_deref().and_then(normalize_label),
            ..CardQuery::default()
        };
        if let Some(slug) = &filter.column {
            query.column_id = Some(column_by_slug(&ctx.columns, slug)?.id.clone());
        }
        if let Some(handle) = &filter.assignee {
            query.assignee = Some(ctx.agent_by_handle(handle)?);
        } else if filter.mine {
            query.assignee = Some(me.cloned().ok_or_else(|| {
                BoardError::InvalidRequest("--mine só vale num terminal de agente.".into())
            })?);
        }
        let cards: Vec<Card> = self
            .store()
            .list_cards(team_id, &query)
            .await?
            .into_iter()
            .filter(|c| {
                filter.include_done
                    || filter.column.is_some()
                    || ctx
                        .column(&c.column_id)
                        .is_none_or(|col| col.kind != ColumnKind::Terminal)
            })
            .collect();
        self.views(&ctx, cards).await
    }

    pub async fn show(&self, team_id: &TeamId, id: &str) -> BoardResult<CardDetail> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        let edges = self.store().dependencies(team_id).await?;
        let all = self
            .store()
            .list_cards(
                team_id,
                &CardQuery {
                    include_archived: true,
                    ..CardQuery::default()
                },
            )
            .await?;
        let reference = |id: &CardId| -> Option<CardRef> {
            all.iter().find(|c| &c.id == id).map(|c| CardRef {
                id: c.id.clone(),
                title: c.title.clone(),
                column_slug: ctx.slug(&c.column_id),
                open: is_open(c, &ctx.columns),
            })
        };
        let depends_on = edges
            .iter()
            .filter(|(t, _)| t == &card.id)
            .filter_map(|(_, d)| reference(d))
            .collect();
        let dependents = edges
            .iter()
            .filter(|(_, d)| d == &card.id)
            .filter_map(|(t, _)| reference(t))
            .collect();
        let children = all
            .iter()
            .filter(|c| c.parent_id.as_ref() == Some(&card.id))
            .filter_map(|c| reference(&c.id))
            .collect();
        let column = ctx
            .column(&card.column_id)
            .cloned()
            .ok_or_else(|| RepoError::Corrupt(format!("tasks.column_id of {}", card.id)))?;
        let comments = self.store().list_comments(&card.id).await?;
        let activity = self.store().list_activity(&card.id).await?;
        let view = self.view(&ctx, card).await?;
        Ok(CardDetail {
            card: view,
            column,
            comments,
            activity,
            depends_on,
            dependents,
            children,
            agents: ctx.tags(),
        })
    }

    /// O próximo cartão que `me` deveria pegar: primeiro os seus que estão prontos, depois
    /// os sem responsável sem dependência aberta; prioridade, depois posição.
    pub async fn next(&self, team_id: &TeamId, me: &AgentId) -> BoardResult<Option<CardView>> {
        let ctx = self.ctx(team_id).await?;
        let ready: Vec<&ColumnId> = ctx
            .columns
            .iter()
            .filter(|c| c.kind == ColumnKind::Ready)
            .map(|c| &c.id)
            .collect();
        let cards = self
            .store()
            .list_cards(team_id, &CardQuery::default())
            .await?;
        let candidates: Vec<Card> = cards
            .into_iter()
            .filter(|c| ready.contains(&&c.column_id))
            .filter(|c| c.assignee.is_none() || c.assignee.as_ref() == Some(me))
            .collect();
        let mut views = self.views(&ctx, candidates).await?;
        views.retain(|v| v.blocked_by.is_empty());
        views.sort_by(|a, b| {
            let mine = |v: &CardView| v.card.assignee.as_ref() == Some(me);
            mine(b)
                .cmp(&mine(a))
                .then(b.card.priority.cmp(&a.card.priority))
                .then(a.card.position.cmp(&b.card.position))
                .then(a.card.id.as_str().cmp(b.card.id.as_str()))
        });
        Ok(views.into_iter().next())
    }

    /// Mudanças desde um instante ("N cartões mudaram desde que você saiu").
    pub async fn changes_since(
        &self,
        team_id: &TeamId,
        since: Millis,
    ) -> BoardResult<Vec<Activity>> {
        Ok(self.store().activity_since(team_id, since).await?)
    }

    /// Espera até algo mudar num cartão que diz respeito a `me` (`aisense task watch`).
    pub async fn watch(&self, me: &AgentId, timeout: Duration) -> Option<BoardEvent> {
        let mut rx = self.hub.subscribe();
        let wait = async {
            loop {
                match rx.recv().await {
                    Ok(event) if event.involved.contains(me) && event.actor.agent() != Some(me) => {
                        return Some(event)
                    }
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        };
        tokio::time::timeout(timeout, wait).await.ok().flatten()
    }

    // ───────────────────────────── escrita ─────────────────────────────

    pub async fn add(&self, team_id: &TeamId, actor: &Actor, new: NewCard) -> BoardResult<Moved> {
        self.add_at(team_id, actor, new, 0).await
    }

    async fn add_at(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        new: NewCard,
        depth: u32,
    ) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        check_text(&new.title, &new.body)?;
        let now = now_ms();
        let existing = self
            .store()
            .list_cards(
                team_id,
                &CardQuery {
                    include_archived: true,
                    ..CardQuery::default()
                },
            )
            .await?;
        if let Some(agent) = actor.agent() {
            let recent = existing
                .iter()
                .filter(|c| c.created_by.as_ref() == Some(agent) && c.created_at > now - 3_600_000)
                .count();
            if recent >= CARDS_PER_AGENT_PER_HOUR {
                return Err(BoardError::InvalidRequest(format!(
                    "Você já criou {recent} cartões na última hora. Agrupe o trabalho em menos cartões (use checklist) ou peça ao humano."
                )));
            }
        }
        let column = match &new.column {
            Some(slug) => column_by_slug(&ctx.columns, slug)?,
            None => first_of_kind(&ctx.columns, ColumnKind::Ready)
                .or_else(|| ctx.columns.first())
                .ok_or_else(|| BoardError::InvalidRequest("o quadro não tem colunas".into()))?,
        };
        if column.requires_approval {
            return Err(BoardError::ApprovalRequired {
                column: column.name.clone(),
            });
        }
        let assignee = match new.assignee.as_deref().filter(|a| !a.trim().is_empty()) {
            Some(handle) => Some(ctx.agent_by_handle(handle)?),
            None => None,
        };
        let parent = match &new.parent {
            Some(raw) => Some(self.find(team_id, raw).await?.id),
            None => None,
        };
        let mut deps = Vec::new();
        for raw in &new.blocked_by {
            deps.push(self.find(team_id, raw).await?.id);
        }
        let checklist = new
            .checklist
            .iter()
            .flat_map(|i| i.split(','))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|text| ChecklistItem {
                text: text.to_owned(),
                done: false,
            })
            .collect();
        let card = Card {
            id: CardId::new(),
            team_id: team_id.clone(),
            column_id: column.id.clone(),
            title: new.title.trim().to_owned(),
            body: new.body.clone(),
            assignee,
            created_by: actor.agent().cloned(),
            parent_id: parent,
            position: existing
                .iter()
                .filter(|c| c.column_id == column.id)
                .map(|c| c.position)
                .max()
                .map_or(0, |p| p + 1),
            priority: new.priority.unwrap_or_default(),
            labels: labels_of(&new.labels)?,
            checklist,
            links: Vec::new(),
            block_reason: new.reason.clone().filter(|r| !r.trim().is_empty()),
            version: 1,
            archived_at: None,
            approved_by: None,
            approved_at: None,
            column_since: now,
            created_at: now,
            updated_at: now,
        };
        check_transition(&card, column, new.reason.as_deref())?;
        if check_wip(column, &existing, &card.id, card.assignee.as_ref()) != Wip::Fits {
            return Err(self.wip_error(&ctx, &card).await?);
        }
        self.store().insert_card(&card).await?;
        let edges = self.store().dependencies(team_id).await?;
        for dep in &deps {
            check_dependency(&edges, &card.id, dep)?;
            self.store().add_dependency(&card.id, dep).await?;
        }
        self.record(
            &card.id,
            actor,
            "created",
            json!({ "column": column.slug, "title": card.title }),
        )
        .await?;
        if let Some(assignee) = &card.assignee {
            let body = format!(
                "Cartão {} atribuído a você por {}: {}\nVeja com: aisense task show {}",
                card.id,
                ctx.label(actor),
                card.title,
                card.id
            );
            self.notify_agent(&ctx, assignee, actor, body).await;
        }
        self.emit(&ctx, Some(&card), None, "created", actor);
        for trigger in [Trigger::CardCreated, Trigger::CardEnters] {
            Box::pin(self.automate(
                &ctx,
                &card.id,
                Fired {
                    trigger,
                    column: card.column_id.clone(),
                },
                actor,
                depth,
            ))
            .await;
        }
        let card = self
            .store()
            .get_card(&card.id)
            .await?
            .ok_or_else(|| BoardError::UnknownCard(card.id.to_string()))?;
        let warnings = self.open_dependency_warnings(&ctx, &card).await?;
        Ok(Moved {
            card: self.view(&ctx, card).await?,
            warnings,
        })
    }

    async fn open_dependency_warnings(&self, ctx: &Ctx, card: &Card) -> BoardResult<Vec<String>> {
        let active = ctx
            .column(&card.column_id)
            .is_some_and(|c| c.kind == ColumnKind::Active);
        if !active {
            return Ok(Vec::new());
        }
        let view = self.view(ctx, card.clone()).await?;
        Ok(view
            .blocked_by
            .iter()
            .map(|dep| {
                format!(
                    "blocked_by_open: {} depende de {dep} (aberto). Siga assim mesmo ou trabalhe no {dep}.",
                    card.id
                )
            })
            .collect())
    }

    async fn finish(&self, ctx: &Ctx, card: Card) -> BoardResult<Moved> {
        // Automações podem ter mudado o cartão depois da gravação: devolve o estado final.
        let card = self.store().get_card(&card.id).await?.unwrap_or(card);
        let warnings = self.open_dependency_warnings(ctx, &card).await?;
        Ok(Moved {
            card: self.view(ctx, card).await?,
            warnings,
        })
    }

    /// Pega para si — atômico (`docs/13`: `UPDATE ... WHERE assignee IS NULL AND version = ?`).
    pub async fn claim(&self, team_id: &TeamId, me: &AgentId, id: &str) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        let actor = Actor::Agent {
            agent_id: me.clone(),
        };
        let now = now_ms();
        let card = self
            .update_with(&ctx, &card.id, &actor, 0, |card, ctx| {
                if card.archived_at.is_some() {
                    return Err(BoardError::Archived(card.id.clone()));
                }
                match &card.assignee {
                    Some(owner) if owner == me => Ok(None),
                    Some(owner) => Err(BoardError::AlreadyClaimed {
                        card: card.id.clone(),
                        by: ctx
                            .handle(owner)
                            .map_or_else(|| "outro agente".into(), |h| format!("@{h}")),
                        secs: ago(now, card.updated_at),
                    }),
                    None => {
                        card.assignee = Some(me.clone());
                        Ok(Some(Change {
                            require_unassigned: true,
                            ..Change::new("assigned", json!({ "assignee": [null, me] }))
                        }))
                    }
                }
            })
            .await?;
        self.finish(&ctx, card).await
    }

    /// Muda de coluna. Gate de aprovação só se passa por `approve`.
    pub async fn move_card(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        slug: &str,
        reason: Option<&str>,
    ) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        let to = column_by_slug(&ctx.columns, slug)?.clone();
        if to.requires_approval && card.column_id != to.id {
            return Err(BoardError::ApprovalRequired { column: to.name });
        }
        if !to.requires_commands.is_empty() && card.column_id != to.id {
            self.run_gate(&ctx, &card, &to, actor).await?;
        }
        let card = self.move_to(&ctx, &card.id, &to, actor, reason, 0).await?;
        self.finish(&ctx, card).await
    }

    async fn move_to(
        &self,
        ctx: &Ctx,
        card_id: &CardId,
        to: &Column,
        actor: &Actor,
        reason: Option<&str>,
        depth: u32,
    ) -> BoardResult<Card> {
        self.update_with(ctx, card_id, actor, depth, |card, ctx| {
            if card.column_id == to.id {
                return Ok(None);
            }
            check_transition(card, to, reason)?;
            let from = ctx.slug(&card.column_id);
            card.column_id = to.id.clone();
            let mut detail = json!({ "from": from, "to": to.slug });
            if to.kind == ColumnKind::Blocked {
                if let Some(reason) = reason.filter(|r| !r.trim().is_empty()) {
                    card.block_reason = Some(reason.trim().to_owned());
                }
                detail["reason"] = json!(card.block_reason);
            }
            // Agente que põe cartão sem dono em andamento vira o dono — antes de gravar, para o
            // WIP por agente valer.
            if to.kind == ColumnKind::Active && card.assignee.is_none() {
                if let Some(agent) = actor.agent() {
                    card.assignee = Some(agent.clone());
                    detail["assignee"] = json!([null, agent]);
                }
            }
            let action = if to.kind == ColumnKind::Blocked {
                "blocked"
            } else {
                "moved"
            };
            Ok(Some(Change::new(action, detail)))
        })
        .await
    }

    pub async fn update(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        patch: CardPatch,
    ) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        if let Some(title) = &patch.title {
            check_text(title, patch.body.as_deref().unwrap_or(""))?;
        } else if let Some(body) = &patch.body {
            check_text("x", body)?;
        }
        let assignee = match patch.assignee.as_deref() {
            None => None,
            Some(raw) if raw.trim().is_empty() => Some(None),
            Some(raw) => Some(Some(ctx.agent_by_handle(raw)?)),
        };
        let add = labels_of(&patch.add_labels)?;
        let remove = labels_of(&patch.remove_labels)?;
        let mut deps_add = Vec::new();
        for raw in &patch.blocked_by {
            deps_add.push(self.find(team_id, raw).await?.id);
        }
        let mut deps_remove = Vec::new();
        for raw in &patch.unblock {
            deps_remove.push(self.find(team_id, raw).await?.id);
        }
        if !deps_add.is_empty() {
            let edges = self.store().dependencies(team_id).await?;
            for dep in &deps_add {
                check_dependency(&edges, &card.id, dep)?;
            }
        }
        let new_items: Vec<ChecklistItem> = patch
            .add_checklist
            .iter()
            .flat_map(|i| i.split(','))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| ChecklistItem {
                text: t.to_owned(),
                done: false,
            })
            .collect();
        let card = self
            .update_with(&ctx, &card.id, actor, 0, |card, _| {
                let mut diff = serde_json::Map::new();
                if let Some(title) = &patch.title {
                    let title = title.trim().to_owned();
                    if title != card.title {
                        diff.insert("title".into(), json!([card.title, title]));
                        card.title = title;
                    }
                }
                if let Some(body) = &patch.body {
                    if body != &card.body {
                        diff.insert("body".into(), json!([card.body, body]));
                        card.body = body.clone();
                    }
                }
                if let Some(assignee) = &assignee {
                    if assignee != &card.assignee {
                        diff.insert("assignee".into(), json!([card.assignee, assignee]));
                        card.assignee = assignee.clone();
                    }
                }
                if let Some(priority) = patch.priority {
                    if priority != card.priority {
                        diff.insert("priority".into(), json!([card.priority, priority]));
                        card.priority = priority;
                    }
                }
                let before = card.labels.clone();
                card.labels.retain(|l| !remove.contains(l));
                for l in &add {
                    if !card.labels.contains(l) {
                        card.labels.push(l.clone());
                    }
                }
                if card.labels != before {
                    diff.insert("labels".into(), json!([before, card.labels]));
                }
                if !new_items.is_empty() {
                    card.checklist.extend(new_items.iter().cloned());
                    diff.insert("checklist".into(), json!({ "added": new_items.len() }));
                }
                if diff.is_empty() {
                    return Ok(None);
                }
                let action = if diff.len() == 1 && diff.contains_key("assignee") {
                    "assigned"
                } else {
                    "updated"
                };
                Ok(Some(Change::new(action, serde_json::Value::Object(diff))))
            })
            .await?;
        for dep in &deps_add {
            self.store().add_dependency(&card.id, dep).await?;
            self.record(&card.id, actor, "dependency", json!({ "added": dep }))
                .await?;
        }
        for dep in &deps_remove {
            self.store().remove_dependency(&card.id, dep).await?;
            self.record(&card.id, actor, "dependency", json!({ "removed": dep }))
                .await?;
        }
        if !deps_add.is_empty() || !deps_remove.is_empty() {
            self.emit(&ctx, Some(&card), None, "dependency", actor);
        }
        self.finish(&ctx, card).await
    }

    /// Marca (ou desmarca) o item `index` (a partir de 1) do checklist.
    pub async fn check(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        index: usize,
        done: bool,
    ) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        let card = self
            .update_with(&ctx, &card.id, actor, 0, |card, _| {
                let total = card.checklist.len();
                let item = index
                    .checked_sub(1)
                    .and_then(|i| card.checklist.get_mut(i))
                    .ok_or_else(|| {
                        BoardError::InvalidRequest(format!(
                            "O checklist de {} tem {total} item(ns); use um número de 1 a {total}.",
                            card.id
                        ))
                    })?;
                if item.done == done {
                    return Ok(None);
                }
                item.done = done;
                let text = item.text.clone();
                Ok(Some(Change::new(
                    "checked",
                    json!({ "item": index, "text": text, "done": done }),
                )))
            })
            .await?;
        self.finish(&ctx, card).await
    }

    pub async fn comment(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        body: &str,
    ) -> BoardResult<Comment> {
        self.comment_at(team_id, actor, id, body, 0).await
    }

    async fn comment_at(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        body: &str,
        depth: u32,
    ) -> BoardResult<Comment> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        if body.trim().is_empty() {
            return Err(BoardError::InvalidRequest(
                "O comentário está vazio.".into(),
            ));
        }
        check_text("x", body)?;
        let comment = Comment {
            id: CommentId::new(),
            card_id: card.id.clone(),
            author: actor.clone(),
            body: body.trim().to_owned(),
            created_at: now_ms(),
        };
        let earlier = self.store().list_comments(&card.id).await?;
        self.store().insert_comment(&comment).await?;
        self.record(
            &card.id,
            actor,
            "commented",
            json!({ "comment": comment.id }),
        )
        .await?;
        // Responsável + criador + quem já comentou, menos quem escreveu agora.
        let mut who: Vec<AgentId> = Vec::new();
        for id in card
            .assignee
            .iter()
            .chain(card.created_by.iter())
            .chain(earlier.iter().filter_map(|c| c.author.agent()))
        {
            if !who.contains(id) {
                who.push(id.clone());
            }
        }
        let text = format!(
            "{} comentou em {} ({}):\n{}",
            ctx.label(actor),
            card.id,
            card.title,
            comment.body
        );
        for agent in &who {
            self.notify_agent(&ctx, agent, actor, text.clone()).await;
        }
        let human_involved =
            card.created_by.is_none() || earlier.iter().any(|c| c.author == Actor::Human);
        if *actor != Actor::Human && human_involved && *actor != Actor::System {
            self.notify(team_id, Address::Human, text).await;
        }
        self.emit(&ctx, Some(&card), None, "commented", actor);
        Box::pin(self.automate(
            &ctx,
            &card.id,
            Fired {
                trigger: Trigger::CommentAdded,
                column: card.column_id.clone(),
            },
            actor,
            depth,
        ))
        .await;
        Ok(comment)
    }

    pub async fn link(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        kind: LinkKind,
        target: &str,
    ) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        let target = target.trim().to_owned();
        if target.is_empty() || target.len() > 2048 {
            return Err(BoardError::InvalidRequest(
                "Diga o que ligar: --pr 42, --commit a1b2c3d, --file caminho ou --url https://…"
                    .into(),
            ));
        }
        let card = self
            .update_with(&ctx, &card.id, actor, 0, |card, _| {
                let link = CardLink {
                    kind,
                    target: target.clone(),
                };
                if card.links.contains(&link) {
                    return Ok(None);
                }
                card.links.push(link.clone());
                Ok(Some(Change::new("linked", json!(link))))
            })
            .await?;
        self.finish(&ctx, card).await
    }

    /// Manda para a coluna de bloqueio, com motivo obrigatório.
    pub async fn block(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        reason: &str,
    ) -> BoardResult<Moved> {
        if reason.trim().is_empty() {
            return Err(BoardError::ReasonRequired);
        }
        let ctx = self.ctx(team_id).await?;
        let to = first_of_kind(&ctx.columns, ColumnKind::Blocked)
            .ok_or_else(|| {
                BoardError::InvalidRequest("Este quadro não tem coluna de bloqueio.".into())
            })?
            .slug
            .clone();
        self.move_card(team_id, actor, id, &to, Some(reason)).await
    }

    /// Conclui: vai para a primeira coluna terminal. A nota vira comentário.
    pub async fn done(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        note: Option<&str>,
    ) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        let to = first_of_kind(&ctx.columns, ColumnKind::Terminal)
            .ok_or_else(|| {
                BoardError::InvalidRequest("Este quadro não tem coluna de concluídos.".into())
            })?
            .slug
            .clone();
        let moved = self.move_card(team_id, actor, id, &to, None).await?;
        if let Some(note) = note.filter(|n| !n.trim().is_empty()) {
            self.comment(team_id, actor, moved.card.card.id.as_str(), note)
                .await?;
        }
        Ok(moved)
    }

    /// Cria subtarefas (filhas) na primeira coluna `ready`, herdando labels e prioridade.
    pub async fn split(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        titles: &[String],
    ) -> BoardResult<Vec<CardView>> {
        let parent = self.find(team_id, id).await?;
        if titles.is_empty() {
            return Err(BoardError::InvalidRequest(
                "Diga as partes: aisense task split <id> \"parte 1\" \"parte 2\"".into(),
            ));
        }
        let mut out = Vec::new();
        for title in titles {
            let moved = self
                .add(
                    team_id,
                    actor,
                    NewCard {
                        title: title.clone(),
                        labels: parent.labels.clone(),
                        priority: Some(parent.priority),
                        parent: Some(parent.id.to_string()),
                        ..NewCard::default()
                    },
                )
                .await?;
            out.push(moved.card);
        }
        Ok(out)
    }

    pub async fn archive(&self, team_id: &TeamId, actor: &Actor, id: &str) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        let now = now_ms();
        let card = self
            .update_with(&ctx, &card.id, actor, 0, |card, _| {
                if card.archived_at.is_some() {
                    return Ok(None);
                }
                card.archived_at = Some(now);
                Ok(Some(Change::new("archived", json!({}))))
            })
            .await?;
        self.finish(&ctx, card).await
    }

    // ─────────────────────────── gate de revisão ───────────────────────────

    async fn run_gate(
        &self,
        ctx: &Ctx,
        card: &Card,
        to: &Column,
        actor: &Actor,
    ) -> BoardResult<()> {
        let result = self
            .gate
            .run(&card.team_id, card.assignee.as_ref(), &to.requires_commands)
            .await;
        let Err(failure) = result else {
            return Ok(());
        };
        let body = format!(
            "O comando '{}' falhou ({}) ao entrar em {}.\n\n```\n{}\n```",
            failure.command,
            failure
                .exit
                .map_or_else(|| "sem código de saída".to_owned(), |c| format!("exit {c}")),
            to.name,
            failure.output.trim_end()
        );
        self.store()
            .insert_comment(&Comment {
                id: CommentId::new(),
                card_id: card.id.clone(),
                author: Actor::System,
                body: body.clone(),
                created_at: now_ms(),
            })
            .await?;
        self.record(
            &card.id,
            actor,
            "gate_failed",
            json!({ "command": failure.command, "exit": failure.exit, "column": to.slug }),
        )
        .await?;
        if let Some(assignee) = &card.assignee {
            self.notify_agent(
                ctx,
                assignee,
                &Actor::System,
                format!(
                    "{} não passou no gate de {}: '{}' falhou. A saída está no cartão: aisense task show {}",
                    card.id, to.name, failure.command, card.id
                ),
            )
            .await;
        }
        self.emit(ctx, Some(card), None, "gate_failed", actor);
        Err(BoardError::GateFailed {
            command: failure.command,
            exit: failure.exit,
        })
    }

    /// Para onde `approve` leva: a próxima coluna que exige aprovação; sem ela, a primeira
    /// terminal.
    fn approval_target<'a>(ctx: &'a Ctx, card: &Card) -> Option<&'a Column> {
        let here = ctx.column(&card.column_id).map_or(0, |c| c.position);
        ctx.columns
            .iter()
            .filter(|c| c.position > here && c.requires_approval)
            .min_by_key(|c| c.position)
            .or_else(|| first_of_kind(&ctx.columns, ColumnKind::Terminal))
    }

    fn reviewers(ctx: &Ctx, except: Option<&AgentId>) -> Vec<String> {
        let others: Vec<&Agent> = ctx
            .agents
            .iter()
            .filter(|a| Some(&a.id) != except)
            .collect();
        let preferred: Vec<String> = others
            .iter()
            .filter(|a| {
                let text = format!("{} {}", a.handle.as_str(), a.role).to_lowercase();
                ["revis", "review", "arquitet", "architect", "coorden"]
                    .iter()
                    .any(|w| text.contains(w))
            })
            .map(|a| a.handle.as_str().to_owned())
            .collect();
        if preferred.is_empty() {
            others
                .iter()
                .take(3)
                .map(|a| a.handle.as_str().to_owned())
                .collect()
        } else {
            preferred
        }
    }

    pub async fn approve(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        note: Option<&str>,
    ) -> BoardResult<Moved> {
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        let to = Self::approval_target(&ctx, &card)
            .cloned()
            .ok_or_else(|| BoardError::InvalidRequest("Não há para onde aprovar.".into()))?;
        let is_author = actor.agent().is_some() && actor.agent() == card.assignee.as_ref();
        if to.approver_must_differ && is_author {
            return Err(BoardError::SelfApproval {
                suggest: Self::reviewers(&ctx, card.assignee.as_ref()),
            });
        }
        if !to.requires_commands.is_empty() {
            self.run_gate(&ctx, &card, &to, actor).await?;
        }
        let now = now_ms();
        let note = note
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned);
        let card = self
            .update_with(&ctx, &card.id, actor, 0, |card, ctx| {
                check_transition(card, &to, None)?;
                let from = ctx.slug(&card.column_id);
                card.column_id = to.id.clone();
                card.approved_by = actor.agent().cloned();
                card.approved_at = Some(now);
                Ok(Some(Change::new(
                    "approved",
                    json!({ "from": from, "to": to.slug, "note": note }),
                )))
            })
            .await?;
        if let Some(note) = &note {
            self.comment(team_id, actor, card.id.as_str(), note).await?;
        }
        if let Some(assignee) = &card.assignee {
            self.notify_agent(
                &ctx,
                assignee,
                actor,
                format!("{} aprovou {} ({}).", ctx.label(actor), card.id, card.title),
            )
            .await;
        }
        self.finish(&ctx, card).await
    }

    /// Devolve para a coluna de onde o cartão veio, com motivo obrigatório.
    pub async fn reject(
        &self,
        team_id: &TeamId,
        actor: &Actor,
        id: &str,
        reason: &str,
    ) -> BoardResult<Moved> {
        if reason.trim().is_empty() {
            return Err(BoardError::RejectReasonRequired);
        }
        let ctx = self.ctx(team_id).await?;
        let card = self.find(team_id, id).await?;
        let here = ctx.slug(&card.column_id);
        let history = self.store().list_activity(&card.id).await?;
        let previous = history
            .iter()
            .rev()
            .filter(|a| a.detail.get("to").and_then(|v| v.as_str()) == Some(here.as_str()))
            .find_map(|a| a.detail.get("from").and_then(|v| v.as_str()))
            .and_then(|slug| ctx.columns.iter().find(|c| c.slug == slug))
            .filter(|c| c.id != card.column_id)
            .or_else(|| first_of_kind(&ctx.columns, ColumnKind::Active))
            .cloned()
            .ok_or_else(|| BoardError::InvalidRequest("Não há para onde devolver.".into()))?;
        let reason = reason.trim().to_owned();
        let card = self
            .update_with(&ctx, &card.id, actor, 0, |card, ctx| {
                let from = ctx.slug(&card.column_id);
                card.column_id = previous.id.clone();
                card.approved_by = None;
                card.approved_at = None;
                Ok(Some(Change::new(
                    "rejected",
                    json!({ "from": from, "to": previous.slug, "reason": reason }),
                )))
            })
            .await?;
        self.store()
            .insert_comment(&Comment {
                id: CommentId::new(),
                card_id: card.id.clone(),
                author: actor.clone(),
                body: format!("Rejeitado: {reason}"),
                created_at: now_ms(),
            })
            .await?;
        if let Some(assignee) = &card.assignee {
            self.notify_agent(
                &ctx,
                assignee,
                actor,
                format!(
                    "{} rejeitou {} ({}) e devolveu para {}. Motivo: {reason}",
                    ctx.label(actor),
                    card.id,
                    card.title,
                    previous.name
                ),
            )
            .await;
        }
        self.finish(&ctx, card).await
    }

    // ───────────────────────────── automações ─────────────────────────────

    async fn automate(&self, ctx: &Ctx, card_id: &CardId, fired: Fired, actor: &Actor, depth: u32) {
        let slug = ctx.slug(&fired.column);
        let rules: Vec<Automation> = ctx
            .board
            .automations
            .iter()
            .filter(|a| a.when == fired.trigger)
            .filter(|a| a.column.as_deref().is_none_or(|c| c == slug))
            .cloned()
            .collect();
        if rules.is_empty() {
            return;
        }
        if depth >= AUTOMATION_MAX_DEPTH {
            tracing::warn!(card = %card_id, depth, "automação em laço interrompida");
            self.notify(
                &ctx.board.team_id,
                Address::Human,
                format!(
                    "Automação em laço interrompida no cartão {card_id}: {} automações seguidas. Revise as automações do quadro.",
                    depth
                ),
            )
            .await;
            return;
        }
        for rule in rules {
            for action in &rule.then {
                if let Err(error) = self.act(ctx, card_id, action, actor, depth + 1).await {
                    tracing::info!(%error, card = %card_id, "ação de automação não aplicada");
                    self.notify(
                        &ctx.board.team_id,
                        Address::Human,
                        format!(
                            "A automação \"{}\" não conseguiu agir em {card_id}: {error}",
                            rule.when.as_str()
                        ),
                    )
                    .await;
                }
            }
        }
    }

    async fn act(
        &self,
        ctx: &Ctx,
        card_id: &CardId,
        action: &Action,
        actor: &Actor,
        depth: u32,
    ) -> BoardResult<()> {
        let system = Actor::System;
        match action {
            Action::Assign { assign } => {
                let who = if assign == "actor" {
                    match actor.agent() {
                        Some(agent) => agent.clone(),
                        None => return Ok(()),
                    }
                } else {
                    ctx.agent_by_handle(assign)?
                };
                self.update_with(ctx, card_id, &system, depth, |card, _| {
                    if card.assignee.as_ref() == Some(&who) {
                        return Ok(None);
                    }
                    if assign == "actor" && card.assignee.is_some() {
                        return Ok(None);
                    }
                    let before = card.assignee.replace(who.clone());
                    Ok(Some(Change::new(
                        "assigned",
                        json!({ "assignee": [before, who], "automation": true }),
                    )))
                })
                .await?;
            }
            Action::Notify { notify, message } => {
                let Some(card) = self.store().get_card(card_id).await? else {
                    return Ok(());
                };
                let body = format!("{message}: {} ({})", card.id, card.title);
                let target = match notify.as_str() {
                    "assignee" => card.assignee.clone(),
                    "creator" => card.created_by.clone(),
                    _ => None,
                };
                match notify.as_str() {
                    "assignee" | "creator" => {
                        if let Some(agent) = target {
                            self.notify_agent(ctx, &agent, &system, body).await;
                        } else if notify == "creator" {
                            self.notify(&ctx.board.team_id, Address::Human, body).await;
                        }
                    }
                    "@voce" => self.notify(&ctx.board.team_id, Address::Human, body).await,
                    handle => {
                        let agent = ctx.agent_by_handle(handle)?;
                        self.notify_agent(ctx, &agent, &system, body).await;
                    }
                }
            }
            Action::Move { to } => {
                let column = column_by_slug(&ctx.columns, to)?.clone();
                if column.requires_approval {
                    return Err(BoardError::ApprovalRequired {
                        column: column.name,
                    });
                }
                Box::pin(self.move_to(ctx, card_id, &column, &system, None, depth)).await?;
            }
            Action::AddLabel { add_label } => {
                let label = normalize_label(add_label).ok_or_else(|| {
                    BoardError::InvalidRequest(format!("Label inválida: '{add_label}'."))
                })?;
                self.update_with(ctx, card_id, &system, depth, |card, _| {
                    if card.labels.contains(&label) {
                        return Ok(None);
                    }
                    card.labels.push(label.clone());
                    Ok(Some(Change::new(
                        "updated",
                        json!({ "labels": { "added": label } }),
                    )))
                })
                .await?;
            }
            Action::UnblockDependents { unblock_dependents } => {
                if *unblock_dependents {
                    self.unblock_dependents(ctx, card_id, depth).await?;
                }
            }
            Action::CreateCard {
                create_card,
                column,
            } => {
                Box::pin(self.add_at(
                    &ctx.board.team_id,
                    &system,
                    NewCard {
                        title: create_card.clone(),
                        column: column.clone(),
                        body: format!("Criado por automação a partir de {card_id}."),
                        ..NewCard::default()
                    },
                    depth,
                ))
                .await?;
            }
        }
        Ok(())
    }

    /// Avisa quem dependia do cartão concluído e tira do bloqueio quem estava parado só
    /// por dependência (sem outra aberta), devolvendo para a primeira coluna `ready`.
    async fn unblock_dependents(&self, ctx: &Ctx, card_id: &CardId, depth: u32) -> BoardResult<()> {
        let Some(done) = self.store().get_card(card_id).await? else {
            return Ok(());
        };
        let edges = self.store().dependencies(&done.team_id).await?;
        let dependents: Vec<CardId> = edges
            .iter()
            .filter(|(_, d)| d == card_id)
            .map(|(t, _)| t.clone())
            .collect();
        for dependent in dependents {
            let Some(card) = self.store().get_card(&dependent).await? else {
                continue;
            };
            if card.archived_at.is_some() {
                continue;
            }
            let view = self.view(ctx, card.clone()).await?;
            let freed = view.blocked_by.is_empty();
            if let Some(assignee) = &card.assignee {
                let body = if freed {
                    format!(
                        "{} ({}) foi concluído: {} ({}) não depende de mais nada aberto.",
                        done.id, done.title, card.id, card.title
                    )
                } else {
                    format!(
                        "{} ({}) foi concluído; {} ainda espera {}.",
                        done.id,
                        done.title,
                        card.id,
                        view.blocked_by
                            .iter()
                            .map(CardId::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                self.notify_agent(ctx, assignee, &Actor::System, body).await;
            }
            let in_blocked = ctx
                .column(&card.column_id)
                .is_some_and(|c| c.kind == ColumnKind::Blocked);
            let reason_is_dependency = card
                .block_reason
                .as_deref()
                .is_some_and(|r| r.contains(done.id.as_str()));
            if freed && in_blocked && reason_is_dependency {
                if let Some(ready) = first_of_kind(&ctx.columns, ColumnKind::Ready).cloned() {
                    Box::pin(self.move_to(ctx, &card.id, &ready, &Actor::System, None, depth))
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// `card_stale`: roda de tempos em tempos (o app chama a cada minuto). Cada cartão
    /// parado dispara uma vez por passagem pela coluna.
    pub async fn tick(&self, now: Millis) -> BoardResult<u32> {
        let teams = self.store().list_teams(TeamFilter::default()).await?;
        let mut fired = 0;
        for team in teams {
            let Some(board) = self.store().get_board(&team.id).await? else {
                continue;
            };
            let stale: Vec<(usize, &Automation)> = board
                .automations
                .iter()
                .enumerate()
                .filter(|(_, a)| a.when == Trigger::CardStale)
                .collect();
            if stale.is_empty() {
                continue;
            }
            let ctx = self.ctx(&team.id).await?;
            let cards = self
                .store()
                .list_cards(&team.id, &CardQuery::default())
                .await?;
            for (index, rule) in stale {
                let Some(column) = rule
                    .column
                    .as_deref()
                    .and_then(|slug| ctx.columns.iter().find(|c| c.slug == slug))
                else {
                    continue;
                };
                #[allow(clippy::cast_possible_truncation)]
                let after_ms = (rule.after_h.unwrap_or(0.0) * 3_600_000.0) as i64;
                for card in cards.iter().filter(|c| c.column_id == column.id) {
                    if now - card.column_since < after_ms {
                        continue;
                    }
                    let key = (card.id.clone(), card.column_since, index);
                    if !lock(&self.stale_fired).insert(key) {
                        continue;
                    }
                    fired += 1;
                    for action in &rule.then {
                        if let Err(error) =
                            self.act(&ctx, &card.id, action, &Actor::System, 1).await
                        {
                            tracing::info!(%error, card = %card.id, "card_stale não aplicado");
                        }
                    }
                }
            }
        }
        Ok(fired)
    }

    // ───────────────────────── configuração (F06-10) ─────────────────────────

    pub async fn set_automations(
        &self,
        team_id: &TeamId,
        automations: Vec<Automation>,
    ) -> BoardResult<Board> {
        let ctx = self.ctx(team_id).await?;
        for (i, a) in automations.iter().enumerate() {
            automation::validate(a, &ctx.columns)
                .map_err(|e| BoardError::InvalidRequest(format!("Automação {}: {e}", i + 1)))?;
        }
        self.store()
            .set_automations(&ctx.board.id, &automations)
            .await?;
        self.emit(&ctx, None, None, "automations", &Actor::Human);
        Ok(Board {
            automations,
            ..ctx.board
        })
    }

    /// Troca as colunas do quadro. Coluna removida com cartões precisa de destino em
    /// `moves` (slug removido → slug que fica).
    pub async fn save_columns(
        &self,
        team_id: &TeamId,
        drafts: Vec<ColumnDraft>,
        moves: Vec<(String, String)>,
    ) -> BoardResult<Vec<Column>> {
        let ctx = self.ctx(team_id).await?;
        if drafts.is_empty() {
            return Err(BoardError::InvalidRequest(
                "O quadro precisa de ao menos uma coluna.".into(),
            ));
        }
        let mut seen = HashSet::new();
        for d in &drafts {
            if !valid_slug(&d.slug) {
                return Err(BoardError::InvalidRequest(format!(
                    "Slug inválido: '{}'. Use minúsculas, dígitos e hífen (até 32).",
                    d.slug
                )));
            }
            if d.name.trim().is_empty() {
                return Err(BoardError::InvalidRequest(format!(
                    "A coluna '{}' precisa de um nome.",
                    d.slug
                )));
            }
            if !seen.insert(d.slug.clone()) {
                return Err(BoardError::InvalidRequest(format!(
                    "Duas colunas com o slug '{}'.",
                    d.slug
                )));
            }
            if d.wip_limit == Some(0) || d.wip_per_agent == Some(0) {
                return Err(BoardError::InvalidRequest(
                    "Limite de WIP zero não deixaria nada entrar: deixe vazio para sem limite."
                        .into(),
                ));
            }
        }
        for kind in [ColumnKind::Ready, ColumnKind::Terminal] {
            if !drafts.iter().any(|d| d.kind == kind) {
                return Err(BoardError::InvalidRequest(format!(
                    "O quadro precisa de uma coluna do tipo {}: é {}.",
                    kind.as_str(),
                    if kind == ColumnKind::Ready {
                        "de onde os agentes puxam trabalho"
                    } else {
                        "onde o trabalho termina"
                    }
                )));
            }
        }
        let columns: Vec<Column> = drafts
            .iter()
            .enumerate()
            .map(|(i, d)| Column {
                id: d
                    .id
                    .clone()
                    .filter(|id| ctx.column(id).is_some())
                    .unwrap_or_default(),
                board_id: ctx.board.id.clone(),
                slug: d.slug.clone(),
                name: d.name.trim().to_owned(),
                kind: d.kind,
                wip_limit: d.wip_limit,
                wip_per_agent: d.wip_per_agent,
                position: u32::try_from(i).unwrap_or(u32::MAX),
                requires_approval: d.requires_approval,
                approver_must_differ: d.approver_must_differ,
                requires_commands: d.requires_commands.clone(),
            })
            .collect();
        let kept: HashSet<&ColumnId> = columns.iter().map(|c| &c.id).collect();
        let removed: Vec<&Column> = ctx
            .columns
            .iter()
            .filter(|c| !kept.contains(&c.id))
            .collect();
        let cards = self
            .store()
            .list_cards(
                team_id,
                &CardQuery {
                    include_archived: true,
                    ..CardQuery::default()
                },
            )
            .await?;
        let mut resolved = Vec::new();
        for column in &removed {
            let count = cards.iter().filter(|c| c.column_id == column.id).count();
            if count == 0 {
                continue;
            }
            let target = moves
                .iter()
                .find(|(from, _)| from == &column.slug)
                .and_then(|(_, to)| columns.iter().find(|c| &c.slug == to))
                .ok_or_else(|| {
                    BoardError::InvalidRequest(format!(
                        "A coluna '{}' tem {count} cartão(ões): escolha para onde movê-los.",
                        column.name
                    ))
                })?;
            resolved.push((column.id.clone(), target.id.clone()));
        }
        // Automações acompanham slug renomeado e perdem as que apontavam para coluna removida.
        let renamed: HashMap<String, String> = ctx
            .columns
            .iter()
            .filter_map(|old| {
                columns
                    .iter()
                    .find(|c| c.id == old.id && c.slug != old.slug)
                    .map(|c| (old.slug.clone(), c.slug.clone()))
            })
            .collect();
        let removed_slugs: HashSet<&str> = removed.iter().map(|c| c.slug.as_str()).collect();
        let automations: Vec<Automation> = ctx
            .board
            .automations
            .iter()
            .cloned()
            .map(|mut a| {
                if let Some(slug) = a.column.as_ref().and_then(|s| renamed.get(s)) {
                    a.column = Some(slug.clone());
                }
                for action in &mut a.then {
                    if let Action::Move { to } = action {
                        if let Some(slug) = renamed.get(to) {
                            *to = slug.clone();
                        }
                    }
                }
                a
            })
            .filter(|a| {
                a.column
                    .as_deref()
                    .is_none_or(|s| !removed_slugs.contains(s))
            })
            .filter(|a| automation::validate(a, &columns).is_ok())
            .collect();
        self.store()
            .replace_columns(&ctx.board.id, &columns, &resolved, now_ms())
            .await?;
        self.store()
            .set_automations(&ctx.board.id, &automations)
            .await?;
        self.emit(&ctx, None, None, "columns", &Actor::Human);
        Ok(self.store().list_columns(&ctx.board.id).await?)
    }
}
