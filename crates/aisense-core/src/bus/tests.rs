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
