//! Barramento no app (Fase 05): sobe o servidor do socket com o app, repassa as mensagens
//! para a UI (`bus:message`, `bus:read`) e expõe a linha do tempo e o compositor do humano.

use std::sync::Arc;
use std::time::Duration;

use aisense_core::bus::{
    BusObserver, BusService, ChannelInfo, MessageMeta, MessageView, Sender, UnreadCount,
};
use aisense_core::repo::TokenRepository;
use aisense_core::{now_ms, AgentId, CommandError, DataDir, MessageId, TeamId};
use aisense_ipc::BusHandler;
use aisense_store::Store;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

use super::agents::Supervisor;

pub type Bus = BusService<Store>;

pub const BUS_MESSAGE: &str = "bus:message";
/// Payload: o `AgentId` cujas mensagens foram lidas (o badge dele muda).
pub const BUS_READ: &str = "bus:read";
/// Uma guarda anti-laço barrou um agente (payload `BusBlocked`).
pub const BUS_BLOCKED: &str = "bus:blocked";

/// Retenção de mensagens (`docs/04`, "Retenção"): o prazo vem das Configurações (90 dias
/// por padrão); limpeza na subida e a cada 6 h.
const RETENTION_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

struct TauriBusObserver {
    app: AppHandle,
    push: super::push::PushSink,
}

impl BusObserver for TauriBusObserver {
    fn blocked(&self, event: &aisense_core::bus::BusBlocked) {
        if let Err(error) = self.app.emit(BUS_BLOCKED, event) {
            tracing::warn!(%error, "falha ao avisar a UI sobre bloqueio");
        }
    }

    fn deliveries_changed(&self, ids: &[MessageId], agent_id: &AgentId) {
        // Lida pela caixa: não precisa mais ser digitada.
        self.push.forget(agent_id, ids);
        if let Err(error) = self.app.emit(BUS_READ, agent_id) {
            tracing::warn!(%error, "falha ao avisar a UI sobre mensagens lidas");
        }
    }
}

/// Encerra o servidor quando o app fecha.
pub struct BusShutdown(pub CancellationToken);

pub fn setup(
    app: &AppHandle,
    store: &Store,
    supervisor: &Supervisor,
    push: super::push::PushSink,
    settings: &super::settings::Settings,
) -> Bus {
    let for_state = supervisor.clone();
    let bus = BusService::new(
        Arc::new(store.clone()),
        Arc::new(move |id: &AgentId| for_state.state(id)),
        Arc::new(TauriBusObserver {
            app: app.clone(),
            push,
        }),
    );
    let prefs = settings.get();
    bus.set_limits(prefs.bus.guards.clone(), prefs.bus.ask_default());

    // Nenhuma sessão da execução anterior está viva: os tokens dela não valem mais.
    if let Err(error) = tauri::async_runtime::block_on(store.revoke_all_tokens()) {
        tracing::warn!(%error, "tokens antigos não revogados");
    }

    // Toda mensagem roteada (socket ou UI) vira `bus:message`, já com os nomes.
    let (events, handle) = (bus.clone(), app.clone());
    tauri::async_runtime::spawn(async move {
        let mut rx = events.subscribe();
        loop {
            match rx.recv().await {
                Ok(routed) => match events.event_for(&routed).await {
                    Ok(event) => {
                        if let Err(error) = handle.emit(BUS_MESSAGE, event) {
                            tracing::warn!(%error, "falha ao emitir mensagem para a UI");
                        }
                    }
                    Err(error) => tracing::warn!(%error, "mensagem sem nomes para a UI"),
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(n, "UI perdeu eventos do barramento");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let (retention, settings) = (store.clone(), std::sync::Arc::clone(settings));
    tauri::async_runtime::spawn(async move {
        loop {
            use aisense_core::bus::BusRepository;
            let keep = settings.get().bus.retention_ms();
            match retention.prune_messages(now_ms() - keep).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(n, "mensagens antigas removidas"),
                Err(error) => tracing::warn!(%error, "retenção de mensagens falhou"),
            }
            tokio::time::sleep(RETENTION_EVERY).await;
        }
    });

    bus
}

/// Sobe o socket com o quadro dentro (o handler atende barramento e `aisense task`).
pub fn serve(
    data: &DataDir,
    board: super::board::BoardState,
    proposals: super::proposals::Proposals,
) -> BusShutdown {
    let shutdown = CancellationToken::new();
    let (endpoint, handler, stop) = (
        data.socket(),
        Arc::new(BusHandler::new(board).with_proposals(proposals)),
        shutdown.clone(),
    );
    tauri::async_runtime::spawn(async move {
        if let Err(error) = aisense_ipc::serve(&endpoint, handler, stop).await {
            // Sem socket os agentes não conversam, mas o resto do app funciona.
            tracing::error!(%error, "barramento indisponível");
        }
    });
    BusShutdown(shutdown)
}

fn bus_error(error: aisense_core::bus::BusError) -> CommandError {
    CommandError::new(error.code(), error.to_string(), error.hint())
}

/// Linha do tempo da equipe, mais nova primeiro, antes do cursor.
#[tauri::command]
pub async fn bus_timeline(
    bus: State<'_, Bus>,
    team_id: TeamId,
    before: Option<MessageId>,
    limit: Option<u32>,
) -> Result<Vec<MessageView>, CommandError> {
    bus.timeline_views(&team_id, before.as_ref(), limit.unwrap_or(100))
        .await
        .map_err(bus_error)
}

/// O humano (`@voce`) manda uma mensagem.
#[tauri::command]
pub async fn bus_send(
    bus: State<'_, Bus>,
    team_id: TeamId,
    to: Vec<String>,
    body: String,
) -> Result<Vec<MessageId>, CommandError> {
    let sent = bus
        .send(
            &team_id,
            Sender::Human,
            &to,
            &body,
            None,
            MessageMeta::default(),
        )
        .await
        .map_err(bus_error)?;
    Ok(sent.into_iter().map(|r| r.message.id).collect())
}

/// Canais da equipe com os inscritos (F07-05).
#[tauri::command]
pub async fn channels_list(
    bus: State<'_, Bus>,
    team_id: TeamId,
) -> Result<Vec<ChannelInfo>, CommandError> {
    bus.channels(&team_id).await.map_err(bus_error)
}

/// Cria ou atualiza: tópico e inscritos (`@handle`; vazio = aberto à equipe toda).
#[tauri::command]
pub async fn channel_save(
    bus: State<'_, Bus>,
    team_id: TeamId,
    slug: String,
    topic: String,
    members: Vec<String>,
) -> Result<ChannelInfo, CommandError> {
    bus.save_channel(&team_id, &slug, &topic, &members)
        .await
        .map_err(bus_error)
}

/// Apaga o canal e as mensagens dele.
#[tauri::command]
pub async fn channel_delete(
    bus: State<'_, Bus>,
    team_id: TeamId,
    slug: String,
) -> Result<(), CommandError> {
    bus.delete_channel(&team_id, &slug).await.map_err(bus_error)
}

/// Não lidas por agente da equipe.
#[tauri::command]
pub async fn bus_unread(
    bus: State<'_, Bus>,
    team_id: TeamId,
) -> Result<Vec<UnreadCount>, CommandError> {
    bus.unread(&team_id).await.map_err(bus_error)
}

/// O humano libera a equipe pausada pelo orçamento de mensagens.
#[tauri::command]
pub fn bus_resume(bus: State<'_, Bus>, team_id: TeamId) {
    bus.resume_team(&team_id);
}

/// A equipe está pausada pelo orçamento?
#[tauri::command]
pub fn bus_paused(bus: State<'_, Bus>, team_id: TeamId) -> bool {
    bus.team_paused(&team_id)
}
