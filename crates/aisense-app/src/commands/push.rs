//! Entrega em modo `push` no app (F05-07, ADR 0006). A regra de quando injetar é do core
//! (`bus::PushQueues`); aqui só se liga: estado do detector e mensagens novas entram,
//! texto sai pelo PTY, e a UI é avisada (`bus:injected`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use aisense_core::agent::AgentState;
use aisense_core::bus::{
    inject, push_item, InjectStyle, Injection, PtyWriter, PushInjected, PushQueues,
};
use aisense_core::repo::AgentRepository;
use aisense_core::state::StateConfidence;
use aisense_core::AgentId;
use aisense_store::Store;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use super::bus::Bus;
use super::pty::Manager;
use super::runtimes::Registry;

pub const BUS_INJECTED: &str = "bus:injected";
/// De quanto em quanto tempo quem só esperava o throttle é liberado.
const TICK: Duration = Duration::from_millis(500);

/// O lado que recebe eventos: o observador do supervisor e o do barramento.
#[derive(Clone)]
pub struct PushSink {
    queues: Arc<PushQueues>,
    tx: mpsc::UnboundedSender<Injection>,
}

impl PushSink {
    pub fn state(&self, agent_id: &AgentId, state: AgentState, confidence: StateConfidence) {
        if let Some(injection) = self
            .queues
            .state(agent_id, state, confidence, Instant::now())
        {
            let _ = self.tx.send(injection);
        }
    }

    pub fn forget(&self, agent_id: &AgentId, ids: &[aisense_core::MessageId]) {
        self.queues.forget(agent_id, ids);
    }
}

/// Criado antes do supervisor (o observador dele precisa do `PushSink`); a tarefa que
/// injeta começa em [`start`], quando o barramento já existe.
pub fn channel() -> (PushSink, mpsc::UnboundedReceiver<Injection>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        PushSink {
            queues: Arc::new(PushQueues::default()),
            tx,
        },
        rx,
    )
}

struct PtyOut(Manager);

impl PtyWriter for PtyOut {
    fn write(&self, agent_id: &AgentId, bytes: &[u8]) -> Result<(), String> {
        self.0
            .write(agent_id.as_str(), bytes)
            .map_err(|e| e.to_string())
    }
}

pub fn start(
    app: &AppHandle,
    sink: &PushSink,
    mut rx: mpsc::UnboundedReceiver<Injection>,
    bus: &Bus,
    store: &Store,
    registry: &Registry,
    pty: &Manager,
) {
    // Mensagem nova para quem está em `push`: entra na fila (e sai já, se o agente estiver
    // ocioso de verdade).
    let (queues, tx, events) = (Arc::clone(&sink.queues), sink.tx.clone(), bus.clone());
    tauri::async_runtime::spawn(async move {
        let mut hub = events.subscribe();
        while let Ok(routed) = hub.recv().await {
            let Ok(dir) = events.directory(&routed.message.team_id).await else {
                continue;
            };
            for delivery in &routed.deliveries {
                let store = events.store();
                if let Some(item) =
                    push_item(&**store, &dir, &delivery.agent_id, &routed.message).await
                {
                    if let Some(injection) =
                        queues.enqueue(&delivery.agent_id, item, Instant::now())
                    {
                        let _ = tx.send(injection);
                    }
                }
            }
        }
    });

    let (queues, tx) = (Arc::clone(&sink.queues), sink.tx.clone());
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TICK).await;
            for injection in queues.tick(Instant::now()) {
                let _ = tx.send(injection);
            }
        }
    });

    let (store, registry, writer, app) = (
        store.clone(),
        Arc::clone(registry),
        PtyOut(Arc::clone(pty)),
        app.clone(),
    );
    tauri::async_runtime::spawn(async move {
        while let Some(injection) = rx.recv().await {
            let Ok(Some(agent)) = store.get_agent(&injection.agent_id).await else {
                continue;
            };
            let Some(adapter) = registry.adapter(&agent.adapter_id) else {
                continue;
            };
            let style = InjectStyle {
                prefix: adapter.inject.prefix.clone(),
                submit: adapter.inject.submit.clone(),
                max_chars: adapter.inject.max_chars as usize,
            };
            match inject(&store, &writer, &style, &injection).await {
                Ok(()) => {
                    let payload = PushInjected {
                        agent_id: injection.agent_id.clone(),
                        message_ids: injection.items.iter().map(|i| i.id.clone()).collect(),
                    };
                    if let Err(error) = app.emit(BUS_INJECTED, payload) {
                        tracing::warn!(%error, "falha ao avisar a UI sobre injeção");
                    }
                }
                Err(error) => {
                    tracing::warn!(agent = %injection.agent_id, %error, "injeção falhou; fica na caixa")
                }
            }
        }
    });
}
