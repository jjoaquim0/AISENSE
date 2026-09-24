//! Propostas contra o `InMemoryStore` (F07-02).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use super::*;
use crate::agent::{AgentState, Autonomy};
use crate::bus::{BusService, NoObserver};
use crate::repo::{AgentRepository, InMemoryStore};
use crate::team::{create_team_with_agents, TeamDraft};

async fn setup() -> (ProposalService<InMemoryStore>, Identity, TeamId) {
    let store = Arc::new(InMemoryStore::new());
    let drafts: Vec<AgentDraft> = ["coordenador", "backend"]
        .iter()
        .map(|h| AgentDraft {
            handle: (*h).into(),
            name: (*h).into(),
            adapter_id: "shell".into(),
            ..AgentDraft::default()
        })
        .collect();
    let (team, agents) = create_team_with_agents(
        &*store,
        &TeamDraft {
            name: "Squad".into(),
            workdir: "/tmp".into(),
            ..TeamDraft::default()
        },
        &drafts,
        1,
    )
    .await
    .unwrap();
    let me = Identity {
        agent_id: agents[0].id.clone(),
        team_id: team.id.clone(),
        handle: agents[0].handle.clone(),
        team_name: team.name.clone(),
        session_id: None,
    };
    let bus = BusService::new(
        store,
        Arc::new(|_: &AgentId| AgentState::Idle),
        Arc::new(NoObserver),
    );
    (
        ProposalService::new(bus, Arc::new(NoProposalObserver)),
        me,
        team.id,
    )
}

#[tokio::test]
async fn agente_propoe_criar_agente_e_so_o_humano_executa() {
    let (service, me, team) = setup().await;
    let action = ProposalAction::CreateAgent {
        handle: "@qa".into(),
        name: "QA".into(),
        role: "testa fluxos".into(),
        adapter_id: "shell".into(),
    };
    let proposal = service
        .propose(&me, action, "ninguém testa o fluxo de login")
        .await
        .unwrap();
    // Nada foi criado ainda.
    let store = service.bus.store();
    assert_eq!(store.list_agents(&team).await.unwrap().len(), 2);
    assert_eq!(service.list(&team, true).await.unwrap().len(), 1);
    // O humano foi avisado na linha do tempo.
    let timeline = service.bus.timeline_views(&team, None, 10).await.unwrap();
    assert!(timeline
        .iter()
        .any(|m| m.to == "@voce" && m.body.contains("propõe criar o agente @qa")));

    let decided = service
        .decide(&team, &proposal.id, true, Some("pode"))
        .await
        .unwrap();
    assert_eq!(decided.state, ProposalState::Accepted);
    let agents = store.list_agents(&team).await.unwrap();
    assert!(agents.iter().any(|a| a.handle.as_str() == "qa"));
    // Quem propôs sabe da decisão.
    let inbox = service.bus.inbox(&me.agent_id, true).await.unwrap();
    assert!(inbox[0].message.body.contains("aceitou e aplicou"));
    // Decidir de novo não vale.
    assert_eq!(
        service
            .decide(&team, &proposal.id, false, None)
            .await
            .unwrap_err()
            .code(),
        "already_decided"
    );
}

#[tokio::test]
async fn autonomia_recusa_e_validacao() {
    let (service, me, team) = setup().await;
    let p = service
        .propose(
            &me,
            ProposalAction::SetAutonomy {
                handle: "@backend".into(),
                autonomy: Autonomy::Trusted,
            },
            "roda testes sozinho",
        )
        .await
        .unwrap();
    service
        .decide(&team, &p.id, false, Some("ainda não"))
        .await
        .unwrap();
    let store = service.bus.store();
    let backend = store
        .list_agents(&team)
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.handle.as_str() == "backend")
        .unwrap();
    assert_eq!(backend.autonomy, Autonomy::Ask);
    let inbox = service.bus.inbox(&me.agent_id, true).await.unwrap();
    assert!(inbox[0].message.body.contains("recusou"));
    assert!(inbox[0].message.body.contains("ainda não"));

    let err = service
        .propose(
            &me,
            ProposalAction::CreateAgent {
                handle: "@backend".into(),
                name: "B".into(),
                role: String::new(),
                adapter_id: "shell".into(),
            },
            "mais um",
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("já existe"));
    let err = service
        .propose(
            &me,
            ProposalAction::ChangeColumns {
                change: "coluna QA".into(),
            },
            " ",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_request");
}
