//! Entrega em modo `push` (F05-07, ADR 0006): a mensagem é digitada no terminal do agente,
//! **só** quando é seguro.
//!
//! Todas as condições valem ao mesmo tempo, sem exceção:
//! 1. estado `idle` com confiança **alta** (regex do prompt casando, não só silêncio);
//! 2. nunca em `awaiting_input` — a IA está pedindo algo ao humano;
//! 3. no máximo uma injeção a cada 3 s por agente; o que chega no meio é agrupado;
//! 4. corpo sanitizado: sem ESC, BEL, `\r`, nenhum controle C0/C1 — nada que o terminal
//!    interprete — e numa linha só;
//! 5. o `submit` é do adaptador, nunca do corpo.
//!
//! Confiança baixa não injeta: a mensagem fica na caixa (`pull`) naquela rodada.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model::{DeliveryState, Message};
use super::repo::BusRepository;
use super::service::Directory;
use crate::agent::{AgentState, DeliveryMode};
use crate::ids::{AgentId, MessageId};
use crate::repo::AgentRepository;
use crate::state::StateConfidence;
use crate::time::now_ms;

/// Throttle entre injeções no mesmo agente.
pub const PUSH_THROTTLE: Duration = Duration::from_secs(3);

/// Tira tudo que o terminal poderia interpretar e junta em uma linha. `\n` vira ` / `
/// (uma quebra de linha no stdin submeteria a mensagem pela metade).
pub fn sanitize(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    for c in body.chars() {
        match c {
            '\n' => out.push_str(" / "),
            '\t' => out.push(' '),
            // C0 (inclui ESC, BEL, \r), DEL e C1 (inclui CSI 0x9b e OSC 0x9d de 8 bits).
            c if (c as u32) < 0x20 || c == '\u{7f}' || ('\u{80}'..='\u{9f}').contains(&c) => {}
            c => out.push(c),
        }
    }
    // Espaços repetidos que sobraram da limpeza.
    out.split(' ')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Uma mensagem a injetar, já com o nome de quem mandou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushItem {
    pub id: MessageId,
    pub from: String,
    pub body: String,
}

/// O texto digitado: uma mensagem, ou várias agrupadas. Acima de `max_chars`, só o aviso
/// para ler a caixa (o corpo inteiro continua lá).
pub fn compose(prefix: &str, items: &[PushItem], max_chars: usize, submit: &str) -> String {
    let line = match items {
        [one] => format!("{prefix}Mensagem de {}: {}", one.from, sanitize(&one.body)),
        many => {
            let parts: Vec<_> = many
                .iter()
                .map(|i| format!("{}: {}", i.from, sanitize(&i.body)))
                .collect();
            format!("{prefix}{} mensagens: {}", many.len(), parts.join(" | "))
        }
    };
    let line = if line.chars().count() > max_chars {
        let n = items.len();
        let from: Vec<_> = items.iter().map(|i| i.from.as_str()).collect();
        format!(
            "{prefix}{n} mensagem(ns) longa(s) de {} — leia com: aisense inbox",
            from.join(", ")
        )
    } else {
        line
    };
    format!("{}{submit}", sanitize(&line))
}

/// Quem escreve no terminal (o `PtyManager` no app; um gravador nos testes).
pub trait PtyWriter: Send + Sync + 'static {
    fn write(&self, agent_id: &AgentId, bytes: &[u8]) -> Result<(), String>;
}

/// Regras de injeção de um adaptador (`[inject]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectStyle {
    pub prefix: String,
    pub submit: String,
    pub max_chars: usize,
}

/// Payload de `bus:injected`: a UI mostra o chip "mensagem injetada" no terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct PushInjected {
    pub agent_id: AgentId,
    pub message_ids: Vec<MessageId>,
}

#[derive(Default)]
struct AgentQueue {
    pending: Vec<PushItem>,
    state: AgentState,
    confidence: StateConfidence,
    last_injection: Option<Instant>,
}

/// As filas de `push`, uma por agente.
#[derive(Default)]
pub struct PushQueues {
    agents: Mutex<HashMap<AgentId, AgentQueue>>,
}

/// O que injetar agora, se algo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injection {
    pub agent_id: AgentId,
    pub items: Vec<PushItem>,
}

impl PushQueues {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<AgentId, AgentQueue>> {
        self.agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Mensagem nova para um agente em `push`.
    pub fn enqueue(&self, agent_id: &AgentId, item: PushItem, now: Instant) -> Option<Injection> {
        let mut agents = self.lock();
        let queue = agents.entry(agent_id.clone()).or_default();
        if !queue.pending.iter().any(|p| p.id == item.id) {
            queue.pending.push(item);
        }
        Self::ready(agent_id, queue, now)
    }

    /// O detector mudou o estado do agente.
    pub fn state(
        &self,
        agent_id: &AgentId,
        state: AgentState,
        confidence: StateConfidence,
        now: Instant,
    ) -> Option<Injection> {
        let mut agents = self.lock();
        let queue = agents.entry(agent_id.clone()).or_default();
        queue.state = state;
        queue.confidence = confidence;
        if !state.is_running() {
            // Processo parou: a fila volta a ser só a caixa de entrada.
            queue.pending.clear();
        }
        Self::ready(agent_id, queue, now)
    }

    /// Passagem periódica: libera quem estava só esperando o throttle.
    pub fn tick(&self, now: Instant) -> Vec<Injection> {
        let mut agents = self.lock();
        agents
            .iter_mut()
            .filter_map(|(id, queue)| Self::ready(id, queue, now))
            .collect()
    }

    /// Mensagens lidas por outro caminho (`aisense inbox`) saem da fila.
    pub fn forget(&self, agent_id: &AgentId, ids: &[MessageId]) {
        if let Some(queue) = self.lock().get_mut(agent_id) {
            queue.pending.retain(|p| !ids.contains(&p.id));
        }
    }

    fn ready(agent_id: &AgentId, queue: &mut AgentQueue, now: Instant) -> Option<Injection> {
        let safe = queue.state == AgentState::Idle && queue.confidence == StateConfidence::High;
        let rested = queue
            .last_injection
            .is_none_or(|at| now.duration_since(at) >= PUSH_THROTTLE);
        if queue.pending.is_empty() || !safe || !rested {
            return None;
        }
        queue.last_injection = Some(now);
        // Depois de digitar, o agente vai trabalhar: até o detector dizer de novo que está
        // ocioso, nada mais entra.
        queue.state = AgentState::Busy;
        Some(Injection {
            agent_id: agent_id.clone(),
            items: std::mem::take(&mut queue.pending),
        })
    }
}

/// Monta o item de uma mensagem para `push`, se o destinatário for um agente em `push`.
pub async fn push_item<S>(
    store: &S,
    directory: &Directory,
    recipient: &AgentId,
    message: &Message,
) -> Option<PushItem>
where
    S: AgentRepository,
{
    let agent = store.get_agent(recipient).await.ok()??;
    (agent.delivery_mode == DeliveryMode::Push).then(|| PushItem {
        id: message.id.clone(),
        from: directory.sender(&message.from),
        body: message.body.clone(),
    })
}

/// Digita e marca as entregas como `delivered`. Falha de escrita deixa as mensagens na
/// caixa (`pull`), com o erro registrado na entrega.
pub async fn inject<S: BusRepository>(
    store: &S,
    writer: &dyn PtyWriter,
    style: &InjectStyle,
    injection: &Injection,
) -> Result<(), String> {
    let text = compose(
        &style.prefix,
        &injection.items,
        style.max_chars,
        &style.submit,
    );
    let written = writer.write(&injection.agent_id, text.as_bytes());
    for item in &injection.items {
        let Ok(deliveries) = store.deliveries_of(&item.id).await else {
            continue;
        };
        if let Some(mut d) = deliveries
            .into_iter()
            .find(|d| d.agent_id == injection.agent_id)
        {
            d.attempts += 1;
            match &written {
                Ok(()) if d.state == DeliveryState::Pending => {
                    d.state = DeliveryState::Delivered;
                    d.delivered_at = Some(now_ms());
                }
                Ok(()) => {}
                Err(e) => d.error = Some(e.clone()),
            }
            let _ = store.update_delivery(&d).await;
        }
    }
    written
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn item(id: &str, from: &str, body: &str) -> PushItem {
        PushItem {
            id: MessageId::from_raw(id),
            from: from.into(),
            body: body.into(),
        }
    }

    #[test]
    fn corpus_malicioso_nao_chega_ao_terminal() {
        let corpus = [
            // ESC + CSI: limpar a tela, mover o cursor.
            "\x1b[2J\x1b[Hoi",
            // OSC 52 (clipboard) e OSC 0 (título), com BEL e com ST.
            "\x1b]52;c;cm0gLXJmIH4=\x07texto",
            "\x1b]0;titulo\x1b\\texto",
            // `\r` para submeter no meio, e `\r\n`.
            "sim\ry\r\nrm -rf ~",
            // C1 de 8 bits: CSI (0x9b) e OSC (0x9d).
            "\u{9b}31mvermelho\u{9d}0;x\u{9c}",
            // Backspace, DEL, NUL, Ctrl-C, Ctrl-D.
            "a\x08b\x7fc\x00d\x03e\x04f",
        ];
        for raw in corpus {
            let clean = sanitize(raw);
            assert!(
                clean
                    .chars()
                    .all(|c| !c.is_control() && !('\u{80}'..='\u{9f}').contains(&c)),
                "{raw:?} → {clean:?}"
            );
            assert!(!clean.contains('\x1b') && !clean.contains('\r'));
        }
        assert_eq!(sanitize("linha 1\nlinha 2"), "linha 1 / linha 2");
        let text = compose(
            "[AISENSE] ",
            &[item("m1", "@x", "sim\r\x1b[2J")],
            4000,
            "\r",
        );
        assert_eq!(text.matches('\r').count(), 1, "só o submit do adaptador");
        assert!(text.ends_with('\r'));
    }

    #[test]
    fn agrupa_e_corta_o_que_nao_cabe() {
        let items = [item("a", "@x", "um"), item("b", "@y", "dois")];
        assert_eq!(
            compose("[AISENSE] ", &items, 4000, "\r"),
            "[AISENSE] 2 mensagens: @x: um | @y: dois\r"
        );
        let long = [item("a", "@x", &"z".repeat(100))];
        assert_eq!(
            compose("[AISENSE] ", &long, 50, "\n"),
            "[AISENSE] 1 mensagem(ns) longa(s) de @x — leia com: aisense inbox\n"
        );
    }

    #[test]
    fn so_injeta_ocioso_com_confianca_alta_e_respeita_o_throttle() {
        let q = PushQueues::default();
        let a = AgentId::new();
        let t0 = Instant::now();
        // Ocupado: fica na fila.
        q.state(&a, AgentState::Busy, StateConfidence::High, t0);
        assert!(q.enqueue(&a, item("1", "@x", "um"), t0).is_none());
        // Aguardando o humano: nunca.
        assert!(q
            .state(&a, AgentState::AwaitingInput, StateConfidence::High, t0)
            .is_none());
        // Ocioso só por silêncio: rebaixa para pull.
        assert!(q
            .state(&a, AgentState::Idle, StateConfidence::Low, t0)
            .is_none());
        // Ocioso de verdade: injeta tudo o que acumulou, de uma vez.
        assert!(q.enqueue(&a, item("2", "@y", "dois"), t0).is_none());
        let first = q
            .state(&a, AgentState::Idle, StateConfidence::High, t0)
            .unwrap();
        assert_eq!(first.items.len(), 2);
        // Nova mensagem 1 s depois, de novo ocioso: throttle segura.
        q.state(&a, AgentState::Idle, StateConfidence::High, t0);
        let t1 = t0 + Duration::from_secs(1);
        assert!(q.enqueue(&a, item("3", "@x", "três"), t1).is_none());
        assert!(q.tick(t1).is_empty());
        // Passados 3 s, o tick libera.
        let t3 = t0 + PUSH_THROTTLE;
        let later = q.tick(t3);
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].items[0].id.as_str(), "3");
        // Mensagem lida pela caixa sai da fila; agente parado esvazia a fila.
        q.state(&a, AgentState::Busy, StateConfidence::High, t3);
        q.enqueue(&a, item("4", "@x", "q"), t3);
        q.forget(&a, &[MessageId::from_raw("4")]);
        q.enqueue(&a, item("5", "@x", "c"), t3);
        q.state(&a, AgentState::Stopped, StateConfidence::High, t3);
        assert!(q
            .state(
                &a,
                AgentState::Idle,
                StateConfidence::High,
                t3 + PUSH_THROTTLE
            )
            .is_none());
    }
}
