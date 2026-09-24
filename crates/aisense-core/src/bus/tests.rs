//! Roteamento (F05-01): DM, canal, broadcast, humano, destinatário inexistente e parado.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use super::*;
use crate::agent::{create_agent, AgentDraft};
use crate::repo::{InMemoryStore, TeamRepository};
use crate::team::{Team, TeamDraft};

struct Squad {
    store: InMemoryStore,
    team: TeamId,
    ids: Vec<AgentId>,
}

impl Squad {
    fn id(&self, handle: &str) -> AgentId {
        let i = ["backend", "frontend", "revisor"]
            .iter()
            .position(|h| *h == handle)
            .unwrap();
        self.ids[i].clone()
    }
    fn from(&self, handle: &str) -> Sender {
        Sender::Agent {
            agent_id: self.id(handle),
        }
    }
}

async fn team(name: &str, store: &InMemoryStore, handles: &[&str]) -> (TeamId, Vec<AgentId>) {
    let team = Team::create(
        &TeamDraft {
            name: name.into(),
            workdir: "/tmp".into(),
            ..TeamDraft::default()
        },
        1,
    )
    .unwrap();
    store.create_team(&team).await.unwrap();
    let mut ids = Vec::new();
    for h in handles {
        let draft = AgentDraft {
            handle: (*h).into(),
            name: (*h).into(),
            adapter_id: "shell".into(),
            ..AgentDraft::default()
        };
        ids.push(create_agent(store, &team.id, &draft, 1).await.unwrap().id);
    }
    (team.id, ids)
}

async fn squad() -> Squad {
    let store = InMemoryStore::new();
    let (team, ids) = team("Squad", &store, &["backend", "frontend", "revisor"]).await;
    Squad { store, team, ids }
}

fn addr(raw: &str) -> Address {
    Address::parse(raw).unwrap()
}

fn all_running(_: &AgentId) -> bool {
    true
}

async fn send(s: &Squad, from: &str, to: &str, body: &str) -> BusResult<Routed> {
    route(
        &s.store,
        &s.team,
        Outgoing::message(s.from(from), addr(to), body),
        10,
        all_running,
    )
    .await
}

fn recipients(routed: &Routed) -> HashSet<AgentId> {
    routed
        .deliveries
        .iter()
        .map(|d| d.agent_id.clone())
        .collect()
}

#[tokio::test]
async fn mensagem_direta_vai_so_para_o_destinatario_e_cai_na_caixa_dele() {
    let s = squad().await;
    let routed = send(&s, "backend", "@frontend", "contrato subiu")
        .await
        .unwrap();
    assert_eq!(
        routed.message.to,
        Target::Agent {
            agent_id: s.id("frontend")
        }
    );
    assert_eq!(recipients(&routed), HashSet::from([s.id("frontend")]));
    assert!(routed.stopped.is_empty());

    let query = InboxQuery {
        unread_only: true,
        after: None,
        limit: 50,
    };
    let inbox = s.store.inbox(&s.id("frontend"), &query).await.unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].message.body, "contrato subiu");
    assert!(s
        .store
        .inbox(&s.id("revisor"), &query)
        .await
        .unwrap()
        .is_empty());

    let read = s
        .store
        .mark_read(
            &s.id("frontend"),
            std::slice::from_ref(&routed.message.id),
            20,
        )
        .await
        .unwrap();
    assert_eq!(read, 1);
    assert!(s
        .store
        .inbox(&s.id("frontend"), &query)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn canal_nasce_no_primeiro_uso_e_vai_para_todos_menos_o_remetente() {
    let s = squad().await;
    let first = send(&s, "backend", "#deploys", "subindo v2").await.unwrap();
    let second = send(&s, "revisor", "#deploys", "ok").await.unwrap();
    let channels = s.store.list_channels(&s.team).await.unwrap();
    assert_eq!(channels.len(), 1, "um canal só, reaproveitado");
    assert_eq!(channels[0].slug, "deploys");
    assert_eq!(first.message.to, second.message.to);
    assert_eq!(
        recipients(&first),
        HashSet::from([s.id("frontend"), s.id("revisor")])
    );
    assert_eq!(
        recipients(&second),
        HashSet::from([s.id("backend"), s.id("frontend")])
    );
}

#[tokio::test]
async fn broadcast_do_humano_vai_para_a_equipe_inteira() {
    let s = squad().await;
    let routed = route(
        &s.store,
        &s.team,
        Outgoing::message(Sender::Human, addr("@all"), "reunião às 15h"),
        10,
        all_running,
    )
    .await
    .unwrap();
    assert_eq!(routed.message.to, Target::Team);
    assert_eq!(recipients(&routed).len(), 3);
}

#[tokio::test]
async fn mensagem_para_o_humano_nao_gera_entrega_de_agente() {
    let s = squad().await;
    let routed = send(&s, "revisor", "@voce", "preciso da sua decisão")
        .await
        .unwrap();
    assert_eq!(routed.message.to, Target::Human);
    assert!(routed.deliveries.is_empty());
    let timeline = s.store.timeline(&s.team, None, 10).await.unwrap();
    assert_eq!(timeline.len(), 1, "aparece na linha do tempo");
}

#[tokio::test]
async fn destinatario_inexistente_e_recusado_com_dica() {
    let s = squad().await;
    let err = send(&s, "backend", "@designer", "oi").await.unwrap_err();
    assert_eq!(err.code(), "unknown_agent");
    assert!(err.hint().unwrap().contains("aisense agents"));
    assert!(s
        .store
        .timeline(&s.team, None, 10)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn destinatario_parado_recebe_recado_mas_pergunta_e_recusada() {
    let s = squad().await;
    let frontend = s.id("frontend");
    let running = |id: &AgentId| id != &frontend;
    let routed = route(
        &s.store,
        &s.team,
        Outgoing::message(s.from("backend"), addr("@frontend"), "leia quando voltar"),
        10,
        running,
    )
    .await
    .unwrap();
    assert_eq!(routed.stopped, [Handle::parse("frontend").unwrap()]);
    assert_eq!(routed.deliveries[0].state, DeliveryState::Pending);

    let ask = Outgoing {
        kind: MessageKind::Request,
        ..Outgoing::message(s.from("backend"), addr("@frontend"), "está aí?")
    };
    let err = route(&s.store, &s.team, ask, 11, running)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "agent_stopped");
    assert!(err.hint().unwrap().contains("aisense send @frontend"));
}

#[tokio::test]
async fn mensagens_nao_atravessam_equipes() {
    let s = squad().await;
    let (_, other) = team("Infra", &s.store, &["sre"]).await;
    let err = route(
        &s.store,
        &s.team,
        Outgoing::message(
            Sender::Agent {
                agent_id: other[0].clone(),
            },
            addr("@backend"),
            "oi",
        ),
        10,
        all_running,
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "invalid_request");
    // E um handle de outra equipe não existe aqui.
    assert_eq!(
        send(&s, "backend", "@sre", "oi").await.unwrap_err().code(),
        "unknown_agent"
    );
}

#[tokio::test]
async fn validacoes_de_corpo_destino_e_resposta() {
    let s = squad().await;
    assert_eq!(
        send(&s, "backend", "@backend", "eu")
            .await
            .unwrap_err()
            .code(),
        "invalid_request"
    );
    assert_eq!(
        send(&s, "backend", "@frontend", "  \n")
            .await
            .unwrap_err()
            .code(),
        "invalid_request"
    );
    let big = "x".repeat(MESSAGE_BODY_MAX + 1);
    assert_eq!(
        send(&s, "backend", "@frontend", &big)
            .await
            .unwrap_err()
            .code(),
        "invalid_request"
    );
    let ask_all = Outgoing {
        kind: MessageKind::Request,
        ..Outgoing::message(s.from("backend"), addr("@all"), "alguém?")
    };
    assert_eq!(
        route(&s.store, &s.team, ask_all, 10, all_running)
            .await
            .unwrap_err()
            .code(),
        "invalid_request"
    );
    let orphan = Outgoing {
        kind: MessageKind::Response,
        reply_to: Some(MessageId::new()),
        ..Outgoing::message(s.from("backend"), addr("@frontend"), "resposta")
    };
    assert_eq!(
        route(&s.store, &s.team, orphan, 10, all_running)
            .await
            .unwrap_err()
            .code(),
        "unknown_message"
    );
}

#[tokio::test]
async fn resposta_volta_para_quem_perguntou() {
    let s = squad().await;
    let ask = Outgoing {
        kind: MessageKind::Request,
        ..Outgoing::message(s.from("backend"), addr("@revisor"), "revisa o diff?")
    };
    let question = route(&s.store, &s.team, ask, 10, all_running)
        .await
        .unwrap();
    let back = reply_address(&s.store, &question.message).await.unwrap();
    assert_eq!(back, addr("@backend"));
    let answer = Outgoing {
        kind: MessageKind::Response,
        reply_to: Some(question.message.id.clone()),
        ..Outgoing::message(s.from("revisor"), back, "aprovado")
    };
    route(&s.store, &s.team, answer, 11, all_running)
        .await
        .unwrap();
    let replies = s.store.replies_to(&question.message.id).await.unwrap();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].body, "aprovado");
}

#[tokio::test]
async fn linha_do_tempo_pagina_por_cursor_do_mais_novo_para_o_mais_velho() {
    let s = squad().await;
    for i in 0..5 {
        send(&s, "backend", "@frontend", &format!("m{i}"))
            .await
            .unwrap();
    }
    let page1 = s.store.timeline(&s.team, None, 2).await.unwrap();
    let bodies: Vec<_> = page1.iter().map(|m| m.body.as_str()).collect();
    assert_eq!(bodies, ["m4", "m3"]);
    let page2 = s
        .store
        .timeline(&s.team, Some(&page1[1].id), 10)
        .await
        .unwrap();
    let bodies: Vec<_> = page2.iter().map(|m| m.body.as_str()).collect();
    assert_eq!(bodies, ["m2", "m1", "m0"]);
    assert_eq!(
        s.store.unread_counts(&s.team).await.unwrap(),
        [(s.id("frontend"), 5)]
    );
}

// ───────────────────────── ask / reply (F05-06) ─────────────────────────

mod ask {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::agent::AgentState;

    fn service(s: Squad, stopped: Option<AgentId>) -> (BusService<InMemoryStore>, Squad) {
        // O serviço fica com o store da equipe; o `Squad` devolvido só serve para achar ids.
        let shared = Arc::new(s.store);
        let s = Squad {
            store: InMemoryStore::new(),
            team: s.team,
            ids: s.ids,
        };
        let bus = BusService::new(
            Arc::clone(&shared),
            Arc::new(move |id: &AgentId| {
                if Some(id) == stopped.as_ref() {
                    AgentState::Stopped
                } else {
                    AgentState::Idle
                }
            }),
            Arc::new(NoObserver),
        );
        (bus, s)
    }

    async fn me(bus: &BusService<InMemoryStore>, s: &Squad, handle: &str) -> Identity {
        let agent = bus.store().get_agent(&s.id(handle)).await.unwrap().unwrap();
        Identity {
            agent_id: agent.id,
            team_id: s.team.clone(),
            handle: agent.handle,
            team_name: "Squad".into(),
            session_id: None,
        }
    }

    #[tokio::test]
    async fn pergunta_bloqueia_ate_a_resposta_e_destrava() {
        let (bus, s) = service(squad().await, None);
        let backend = me(&bus, &s, "backend").await;
        let revisor = me(&bus, &s, "revisor").await;

        let asking = {
            let (bus, backend) = (bus.clone(), backend.clone());
            tokio::spawn(async move {
                bus.ask(
                    &backend,
                    "@revisor",
                    "revisa o diff?",
                    Some(Duration::from_secs(5)),
                )
                .await
            })
        };
        // O revisor vê a pergunta e responde.
        let question = loop {
            let inbox = bus.inbox(&revisor.agent_id, false).await.unwrap();
            if let Some(item) = inbox.into_iter().next() {
                break item.message;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        };
        assert_eq!(question.kind, MessageKind::Request);
        assert_eq!(question.meta.timeout_s, Some(5));
        bus.reply(&revisor, question.id.as_str(), "aprovado, pode subir")
            .await
            .unwrap();

        let answer = asking.await.unwrap().unwrap();
        assert_eq!(answer.body, "aprovado, pode subir");
        assert_eq!(answer.kind, MessageKind::Response);
        assert_eq!(answer.reply_to, Some(question.id.clone()));
        // Pergunta respondida e resposta recebida contam como lidas.
        assert!(bus
            .inbox(&revisor.agent_id, false)
            .await
            .unwrap()
            .is_empty());
        assert!(bus
            .inbox(&backend.agent_id, false)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn sem_resposta_no_prazo_e_timeout_com_dica() {
        let (bus, s) = service(squad().await, None);
        let backend = me(&bus, &s, "backend").await;
        let err = bus
            .ask(
                &backend,
                "@revisor",
                "está aí?",
                Some(Duration::from_millis(50)),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "timeout");
        assert!(err.to_string().contains("@revisor"));
        assert!(err.hint().unwrap().contains("aisense send"));
    }

    #[tokio::test]
    async fn ask_mutuo_e_recusado_com_would_deadlock() {
        let (bus, s) = service(squad().await, None);
        let backend = me(&bus, &s, "backend").await;
        let frontend = me(&bus, &s, "frontend").await;
        let revisor = me(&bus, &s, "revisor").await;
        // backend → frontend → revisor esperando; revisor perguntar ao backend fecha o ciclo.
        let first = {
            let (bus, backend) = (bus.clone(), backend.clone());
            tokio::spawn(async move {
                bus.ask(&backend, "@frontend", "a", Some(Duration::from_secs(5)))
                    .await
            })
        };
        let second = {
            let (bus, frontend) = (bus.clone(), frontend.clone());
            tokio::spawn(async move {
                bus.ask(&frontend, "@revisor", "b", Some(Duration::from_secs(5)))
                    .await
            })
        };
        // Espera as duas perguntas estarem registradas.
        while bus.store().timeline(&s.team, None, 10).await.unwrap().len() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let err = bus
            .ask(&revisor, "@backend", "c", Some(Duration::from_secs(1)))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "would_deadlock");
        assert!(
            err.to_string().contains("@backend → @frontend → @revisor"),
            "{err}"
        );
        // Mútuo direto também.
        let err = bus
            .ask(&frontend, "@backend", "d", Some(Duration::from_secs(1)))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "would_deadlock");
        first.abort();
        second.abort();
        let _ = (first.await, second.await);
        // Com as esperas encerradas, perguntar volta a valer.
        let third = bus
            .ask(&revisor, "@backend", "e", Some(Duration::from_millis(20)))
            .await
            .unwrap_err();
        assert_eq!(
            third.code(),
            "timeout",
            "sem ciclo agora: só não teve resposta"
        );
    }

    #[tokio::test]
    async fn perguntar_a_quem_esta_parado_falha_na_hora() {
        let s = squad().await;
        let stopped = s.id("frontend");
        let (bus, s) = service(s, Some(stopped));
        let backend = me(&bus, &s, "backend").await;
        let started = std::time::Instant::now();
        let err = bus
            .ask(&backend, "@frontend", "oi?", Some(Duration::from_secs(30)))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "agent_stopped");
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn so_responde_o_que_recebeu() {
        let (bus, s) = service(squad().await, None);
        let backend = me(&bus, &s, "backend").await;
        let revisor = me(&bus, &s, "revisor").await;
        let sent = bus
            .send(
                &s.team,
                Sender::Agent {
                    agent_id: backend.agent_id.clone(),
                },
                &["@frontend".into()],
                "p/ frontend",
                None,
                MessageMeta::default(),
            )
            .await
            .unwrap();
        let err = bus
            .reply(&revisor, sent[0].message.id.as_str(), "intrometido")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_request");
        assert_eq!(
            bus.reply(&revisor, "msg_nao_existe", "x")
                .await
                .unwrap_err()
                .code(),
            "unknown_message"
        );
    }
}

// ───────────────────────── guardas anti-laço (F05-10) ─────────────────────────

mod guards {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::agent::AgentState;

    #[derive(Default)]
    struct Seen(Mutex<Vec<BusBlocked>>);
    impl BusObserver for Seen {
        fn blocked(&self, event: &BusBlocked) {
            self.0.lock().unwrap().push(event.clone());
        }
    }

    fn from(s: &Squad, handle: &str) -> Sender {
        s.from(handle)
    }

    #[tokio::test]
    async fn ping_pong_e_interrompido_e_o_humano_decide() {
        let s = squad().await;
        let (team, a, b) = (s.team.clone(), from(&s, "backend"), from(&s, "frontend"));
        let seen = Arc::new(Seen::default());
        let bus = BusService::new(
            Arc::new(s.store),
            Arc::new(|_: &AgentId| AgentState::Idle),
            Arc::clone(&seen) as Arc<dyn BusObserver>,
        )
        .with_guards(GuardConfig {
            team_per_hour: 6,
            ..GuardConfig::default()
        });
        let mut last: Option<MessageId> = None;
        let mut sent = 0;
        let mut blocked = None;
        for i in 0..20 {
            let (who, to) = if i % 2 == 0 {
                (&a, "@frontend")
            } else {
                (&b, "@backend")
            };
            let out = Outgoing {
                reply_to: last.clone(),
                ..Outgoing::message(who.clone(), addr(to), format!("e aí? {i}"))
            };
            match bus.dispatch(&team, out).await {
                Ok(routed) => {
                    last = Some(routed.message.id);
                    sent += 1;
                }
                Err(e) => {
                    blocked = Some(e);
                    break;
                }
            }
        }
        assert_eq!(sent, 6, "parou no limite configurado");
        let err = blocked.unwrap();
        assert_eq!(err.code(), "team_paused");
        assert!(bus.team_paused(&team));

        // Outra tentativa barrada não gera outro aviso.
        let again = bus
            .dispatch(
                &team,
                Outgoing::message(b.clone(), addr("@backend"), "insisto"),
            )
            .await
            .unwrap_err();
        assert_eq!(again.code(), "team_paused");
        let events = seen.0.lock().unwrap().clone();
        assert_eq!(events.len(), 1);
        assert!(events[0].detail.contains("pausadas até você liberar"));
        let timeline = bus.store().timeline(&team, None, 50).await.unwrap();
        let system: Vec<_> = timeline
            .iter()
            .filter(|m| m.kind == MessageKind::System && m.to == Target::Human)
            .collect();
        assert_eq!(system.len(), 1, "um aviso na linha do tempo");

        // O humano continua falando e libera a equipe.
        bus.dispatch(
            &team,
            Outgoing::message(Sender::Human, addr("@all"), "parem e resumam"),
        )
        .await
        .unwrap();
        bus.resume_team(&team);
        bus.dispatch(&team, Outgoing::message(a, addr("@frontend"), "resumo: X"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn cadeia_longa_avisa_o_remetente_uma_vez() {
        let s = squad().await;
        let (team, a, b) = (s.team.clone(), from(&s, "backend"), from(&s, "frontend"));
        let backend = s.id("backend");
        let bus = BusService::new(
            Arc::new(s.store),
            Arc::new(|_: &AgentId| AgentState::Idle),
            Arc::new(NoObserver),
        )
        .with_guards(GuardConfig {
            max_reply_depth: 4,
            ..GuardConfig::default()
        });
        let mut last: Option<MessageId> = None;
        for i in 0..8 {
            let (who, to) = if i % 2 == 0 {
                (&a, "@frontend")
            } else {
                (&b, "@backend")
            };
            let out = Outgoing {
                reply_to: last.clone(),
                ..Outgoing::message(who.clone(), addr(to), format!("r{i}"))
            };
            last = Some(bus.dispatch(&team, out).await.unwrap().message.id);
        }
        let inbox = bus.inbox(&backend, false).await.unwrap();
        let warnings: Vec<_> = inbox
            .iter()
            .filter(|i| i.message.kind == MessageKind::System)
            .collect();
        assert_eq!(warnings.len(), 1, "um aviso, na profundidade exata");
        assert!(warnings[0].message.body.contains("4 respostas encadeadas"));
    }

    #[tokio::test]
    async fn mensagem_repetida_e_barrada_com_dica() {
        let s = squad().await;
        let (team, a) = (s.team.clone(), from(&s, "backend"));
        let bus = BusService::new(
            Arc::new(s.store),
            Arc::new(|_: &AgentId| AgentState::Idle),
            Arc::new(NoObserver),
        );
        for _ in 0..3 {
            bus.dispatch(
                &team,
                Outgoing::message(a.clone(), addr("@frontend"), "pronto?"),
            )
            .await
            .unwrap();
        }
        let err = bus
            .dispatch(&team, Outgoing::message(a, addr("@frontend"), "pronto?"))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "repeated_message");
        assert!(err.hint().unwrap().contains("aisense note"));
    }
}
