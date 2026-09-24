//! O quadro inteiro contra o `InMemoryStore`: regras, `claim` sob concorrência,
//! dependências, automações, avisos pelo barramento e gate de revisão (Fase 06).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::agent::{AgentDraft, AgentState, DeliveryMode};
use crate::bus::{BusService, NoObserver};
use crate::ids::{AgentId, TeamId};
use crate::repo::InMemoryStore;
use crate::team::{create_team_with_agents, TeamDraft};

struct Squad {
    board: BoardService<InMemoryStore>,
    team: TeamId,
    ids: Vec<(String, AgentId)>,
}

impl Squad {
    fn id(&self, handle: &str) -> AgentId {
        self.ids
            .iter()
            .find(|(h, _)| h == handle)
            .map(|(_, id)| id.clone())
            .unwrap()
    }

    fn agent(&self, handle: &str) -> Actor {
        Actor::Agent {
            agent_id: self.id(handle),
        }
    }

    async fn inbox(&self, handle: &str) -> Vec<String> {
        self.board
            .bus()
            .inbox(&self.id(handle), true)
            .await
            .unwrap()
            .into_iter()
            .map(|i| i.message.body)
            .collect()
    }

    async fn add(&self, title: &str) -> CardId {
        self.board
            .add(
                &self.team,
                &Actor::Human,
                NewCard {
                    title: title.into(),
                    ..NewCard::default()
                },
            )
            .await
            .unwrap()
            .card
            .card
            .id
    }

    async fn column_of(&self, id: &CardId) -> String {
        self.board
            .show(&self.team, id.as_str())
            .await
            .unwrap()
            .column
            .slug
    }
}

/// Gate falso: `test` falha com a saída dada, o resto passa.
#[derive(Default)]
struct FakeGate {
    runs: Mutex<Vec<(Option<AgentId>, Vec<String>)>>,
}

impl Gate for FakeGate {
    fn run<'a>(
        &'a self,
        _team_id: &'a TeamId,
        assignee: Option<&'a AgentId>,
        commands: &'a [String],
    ) -> GateFuture<'a> {
        self.runs
            .lock()
            .unwrap()
            .push((assignee.cloned(), commands.to_vec()));
        Box::pin(async move {
            match commands.iter().find(|c| *c == "test") {
                Some(c) => Err(GateFailure {
                    command: c.clone(),
                    exit: Some(1),
                    output: "assertion failed: refresh invalida o token antigo".into(),
                }),
                None => Ok(()),
            }
        })
    }
}

async fn squad_with(gate: Arc<dyn Gate>) -> Squad {
    let store = Arc::new(InMemoryStore::new());
    let drafts: Vec<AgentDraft> = ["backend", "frontend", "revisor"]
        .iter()
        .map(|h| AgentDraft {
            handle: (*h).into(),
            name: (*h).into(),
            adapter_id: "shell".into(),
            delivery_mode: DeliveryMode::Hook,
            ..AgentDraft::default()
        })
        .collect();
    let (team, agents) = create_team_with_agents(
        &*store,
        &TeamDraft {
            name: "Squad Produto".into(),
            workdir: "/tmp".into(),
            ..TeamDraft::default()
        },
        &drafts,
        1,
    )
    .await
    .unwrap();
    let bus = BusService::new(
        store,
        Arc::new(|_: &AgentId| AgentState::Idle),
        Arc::new(NoObserver),
    );
    Squad {
        board: BoardService::new(bus, Arc::new(NoBoardObserver), gate),
        team: team.id,
        ids: agents
            .into_iter()
            .map(|a| (a.handle.as_str().to_owned(), a.id))
            .collect(),
    }
}

async fn squad() -> Squad {
    squad_with(Arc::new(NoGate)).await
}

// ─────────────────────────── F06-01 ───────────────────────────

#[tokio::test]
async fn equipe_nasce_com_quadro_e_colunas_padrao() {
    let s = squad().await;
    let store = s.board.bus().store();
    let board = store.get_board(&s.team).await.unwrap().expect("quadro");
    let columns = store.list_columns(&board.id).await.unwrap();
    let slugs: Vec<_> = columns.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(
        slugs,
        ["backlog", "todo", "doing", "blocked", "review", "done"]
    );
    // O revisor da equipe já recebe o aviso da coluna de revisão.
    assert!(board.automations.iter().any(|a| a
        .then
        .iter()
        .any(|t| matches!(t, Action::Notify { notify, .. } if notify == "@revisor"))));
    // Idempotente.
    let again = ensure_board(&**store, &s.team, 2).await.unwrap();
    assert_eq!(again.id, board.id);
}

#[tokio::test]
async fn transicao_invalida_devolve_erro_tipado() {
    let s = squad().await;
    let id = s.add("Remover legacy_id").await;
    let err = s
        .board
        .move_card(&s.team, &Actor::Human, id.as_str(), "blocked", None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "reason_required");
    let err = s
        .board
        .move_card(&s.team, &Actor::Human, id.as_str(), "em-progresso", None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "unknown_column");
    assert_eq!(
        err.to_string(),
        "Coluna 'em-progresso' não existe. Colunas: backlog, todo, doing, blocked, review, done."
    );
    let err = s
        .board
        .block(&s.team, &Actor::Human, id.as_str(), "  ")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "reason_required");
    let moved = s
        .board
        .block(
            &s.team,
            &Actor::Human,
            id.as_str(),
            "aguarda decisão do @arquiteto",
        )
        .await
        .unwrap();
    assert_eq!(moved.card.column_slug, "blocked");
    assert_eq!(
        moved.card.card.block_reason.as_deref(),
        Some("aguarda decisão do @arquiteto")
    );
    // Saiu do bloqueio, o motivo sai junto.
    let moved = s
        .board
        .move_card(&s.team, &Actor::Human, id.as_str(), "todo", None)
        .await
        .unwrap();
    assert_eq!(moved.card.card.block_reason, None);
    // O id curto do `aisense board` também serve.
    let short = short_id(id.as_str());
    assert_eq!(
        s.board.show(&s.team, &short).await.unwrap().card.card.id,
        id
    );
}

// ─────────────────────────── F06-03 ───────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn claim_atomico_com_oito_disputando() {
    let s = squad().await;
    let id = s.add("Migrar /users para OAuth").await;
    // 8 agentes (3 da equipe e 5 criados agora) disputam o mesmo cartão ao mesmo tempo.
    let store = s.board.bus().store();
    let mut contenders: Vec<AgentId> = s.ids.iter().map(|(_, id)| id.clone()).collect();
    for i in 0..5 {
        let draft = AgentDraft {
            handle: format!("extra{i}"),
            name: format!("Extra {i}"),
            adapter_id: "shell".into(),
            ..AgentDraft::default()
        };
        contenders.push(
            crate::agent::create_agent(&**store, &s.team, &draft, 1)
                .await
                .unwrap()
                .id,
        );
    }
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut tasks = Vec::new();
    for agent in contenders {
        let (board, team, id, barrier) = (
            s.board.clone(),
            s.team.clone(),
            id.clone(),
            Arc::clone(&barrier),
        );
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            board.claim(&team, &agent, id.as_str()).await
        }));
    }
    let mut won = 0;
    let mut lost = 0;
    for t in tasks {
        match t.await.unwrap() {
            Ok(_) => won += 1,
            Err(e) => {
                assert_eq!(e.code(), "already_claimed", "{e}");
                assert!(e.to_string().contains("já foi pego por @"));
                assert_eq!(e.hint().unwrap(), "Use 'aisense task next' para o próximo.");
                lost += 1;
            }
        }
    }
    assert_eq!((won, lost), (1, 7));
}

#[tokio::test]
async fn wip_aplicado_de_verdade() {
    let s = squad().await;
    let (a, b) = (s.add("A").await, s.add("B").await);
    let backend = s.agent("backend");
    s.board
        .move_card(&s.team, &backend, a.as_str(), "doing", None)
        .await
        .unwrap();
    // Fazendo: 1 por agente.
    let err = s
        .board
        .move_card(&s.team, &backend, b.as_str(), "doing", None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "wip_exceeded");
    assert!(err.to_string().contains("@backend já tem 1"), "{err}");
    assert_eq!(s.column_of(&b).await, "todo");
    // Outro agente pode.
    s.board
        .move_card(&s.team, &s.agent("frontend"), b.as_str(), "doing", None)
        .await
        .unwrap();

    // Limite total da coluna, com a mensagem do doc.
    let store = s.board.bus().store();
    let board = store.get_board(&s.team).await.unwrap().unwrap();
    let mut columns = store.list_columns(&board.id).await.unwrap();
    columns[2].wip_limit = Some(2);
    let drafts = columns
        .iter()
        .map(|c| ColumnDraft {
            id: Some(c.id.clone()),
            slug: c.slug.clone(),
            name: c.name.clone(),
            kind: c.kind,
            wip_limit: c.wip_limit,
            wip_per_agent: c.wip_per_agent,
            requires_approval: c.requires_approval,
            approver_must_differ: c.approver_must_differ,
            requires_commands: c.requires_commands.clone(),
        })
        .collect();
    s.board.save_columns(&s.team, drafts, vec![]).await.unwrap();
    let c = s.add("C").await;
    let err = s
        .board
        .move_card(&s.team, &s.agent("revisor"), c.as_str(), "doing", None)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Fazendo está no limite (2/2). Conclua ou devolva um cartão antes de pegar outro."
    );
}

// ─────────────────────────── F06-04 ───────────────────────────

#[tokio::test]
async fn dependencias_checklist_comentarios_e_historico() {
    let s = squad().await;
    let a = s.add("Migrar /users para OAuth").await;
    let b = s
        .board
        .add(
            &s.team,
            &Actor::Human,
            NewCard {
                title: "Atualizar o cliente TypeScript".into(),
                assignee: Some("@frontend".into()),
                blocked_by: vec![a.to_string()],
                checklist: vec!["escrever teste,implementar".into()],
                ..NewCard::default()
            },
        )
        .await
        .unwrap()
        .card;
    assert_eq!(b.blocked_by, vec![a.clone()]);
    let b = b.card.id;
    // A→B→A é recusado.
    let err = s
        .board
        .update(
            &s.team,
            &Actor::Human,
            a.as_str(),
            CardPatch {
                blocked_by: vec![b.to_string()],
                ..CardPatch::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "dependency_cycle");
    assert_eq!(
        err.to_string(),
        format!("Isso criaria um ciclo: {a} → {b} → {a}.")
    );

    // `next` não oferece cartão com dependência aberta.
    let next = s.board.next(&s.team, &s.id("frontend")).await.unwrap();
    assert_eq!(next.map(|n| n.card.id), Some(a.clone()));

    // Aviso (não bloqueio) ao começar com dependência aberta.
    let moved = s
        .board
        .move_card(&s.team, &s.agent("frontend"), b.as_str(), "doing", None)
        .await
        .unwrap();
    assert!(
        moved.warnings[0].starts_with("blocked_by_open"),
        "{:?}",
        moved.warnings
    );

    // Checklist marcável.
    let checked = s
        .board
        .check(&s.team, &s.agent("frontend"), b.as_str(), 2, true)
        .await
        .unwrap();
    assert_eq!(checked.card.card.checklist_progress(), (1, 2));
    assert_eq!(
        s.board
            .check(&s.team, &Actor::Human, b.as_str(), 9, true)
            .await
            .unwrap_err()
            .code(),
        "invalid_request"
    );

    // Comentário chega ao responsável como mensagem.
    s.inbox("frontend").await;
    s.board
        .comment(
            &s.team,
            &Actor::Human,
            b.as_str(),
            "o contrato mudou, veja o commit a1b2c3d",
        )
        .await
        .unwrap();
    let inbox = s.inbox("frontend").await;
    assert!(
        inbox.iter().any(|m| m.contains("o contrato mudou")),
        "{inbox:?}"
    );

    // Concluir A libera B.
    s.board
        .move_card(&s.team, &s.agent("backend"), a.as_str(), "doing", None)
        .await
        .unwrap();
    s.board
        .done(
            &s.team,
            &s.agent("backend"),
            a.as_str(),
            Some("implementado no a1b2c3d"),
        )
        .await
        .unwrap();
    let detail = s.board.show(&s.team, b.as_str()).await.unwrap();
    assert!(detail.card.blocked_by.is_empty());
    assert!(!detail.depends_on[0].open);
    let inbox = s.inbox("frontend").await;
    assert!(
        inbox
            .iter()
            .any(|m| m.contains("foi concluído") && m.contains("não depende de mais nada")),
        "{inbox:?}"
    );

    // Histórico imutável com autor e diff.
    let actions: Vec<_> = detail.activity.iter().map(|a| a.action.as_str()).collect();
    assert_eq!(actions, ["created", "moved", "checked", "commented"]);
    assert_eq!(detail.activity[1].detail["from"], "todo");
    assert_eq!(detail.activity[1].actor, s.agent("frontend"));
    assert_eq!(detail.comments[0].author, Actor::Human);
}

#[tokio::test]
async fn bloqueado_por_dependencia_volta_quando_ela_termina() {
    let s = squad().await;
    let a = s.add("A").await;
    let b = s.add("B").await;
    s.board
        .update(
            &s.team,
            &Actor::Human,
            b.as_str(),
            CardPatch {
                blocked_by: vec![a.to_string()],
                ..CardPatch::default()
            },
        )
        .await
        .unwrap();
    s.board
        .block(&s.team, &Actor::Human, b.as_str(), &format!("espera {a}"))
        .await
        .unwrap();
    s.board
        .done(&s.team, &Actor::Human, a.as_str(), None)
        .await
        .unwrap();
    assert_eq!(s.column_of(&b).await, "todo");
}

// ─────────────────────────── F06-06 / F06-07 ───────────────────────────

#[tokio::test]
async fn automacoes_movem_trabalho_e_avisam() {
    let s = squad().await;
    let id = s.add("Endpoint /auth/token").await;
    // Card entrando em Fazendo sem dono: quem moveu vira o responsável.
    let moved = s
        .board
        .move_card(&s.team, &s.agent("backend"), id.as_str(), "doing", None)
        .await
        .unwrap();
    assert_eq!(moved.card.assignee_handle.as_deref(), Some("backend"));
    // Entrando em Revisão, o revisor é avisado (modo hook: descobre no fim do turno).
    s.inbox("revisor").await;
    s.board
        .move_card(&s.team, &s.agent("backend"), id.as_str(), "review", None)
        .await
        .unwrap();
    let inbox = s.inbox("revisor").await;
    assert!(
        inbox
            .iter()
            .any(|m| m.starts_with("cartão pronto para revisão")),
        "{inbox:?}"
    );
}

#[tokio::test]
async fn atribuir_avisa_o_responsavel() {
    let s = squad().await;
    let id = s.add("Middleware de refresh token").await;
    s.board
        .update(
            &s.team,
            &Actor::Human,
            id.as_str(),
            CardPatch {
                assignee: Some("@backend".into()),
                ..CardPatch::default()
            },
        )
        .await
        .unwrap();
    let inbox = s.inbox("backend").await;
    assert!(
        inbox[0].starts_with(&format!("Cartão {id} atribuído a você por @voce")),
        "{inbox:?}"
    );
    // O próprio agente pegando não se avisa.
    let other = s.add("Outro").await;
    s.board
        .claim(&s.team, &s.id("backend"), other.as_str())
        .await
        .unwrap();
    assert!(s.inbox("backend").await.is_empty());
}

#[tokio::test]
async fn card_stale_dispara_depois_do_prazo_uma_vez() {
    let s = squad().await;
    let id = s.add("Testes de integração").await;
    s.board
        .move_card(&s.team, &s.agent("backend"), id.as_str(), "doing", None)
        .await
        .unwrap();
    s.inbox("backend").await;
    let now = crate::time::now_ms();
    assert_eq!(s.board.tick(now + 3_600_000).await.unwrap(), 0);
    assert_eq!(s.board.tick(now + 4 * 3_600_000 + 1_000).await.unwrap(), 1);
    assert_eq!(s.board.tick(now + 5 * 3_600_000).await.unwrap(), 0);
    let inbox = s.inbox("backend").await;
    assert!(inbox[0].starts_with("cartão parado há 4h"), "{inbox:?}");
}

#[tokio::test]
async fn automacao_em_laco_e_interrompida() {
    let s = squad().await;
    let loop_rules = automations_from_toml(
        r#"
[[automation]]
when = "card_enters"
column = "doing"
then = [{ move = "review" }]

[[automation]]
when = "card_enters"
column = "review"
then = [{ move = "doing" }]
"#,
    )
    .unwrap();
    s.board.set_automations(&s.team, loop_rules).await.unwrap();
    let id = s.add("Laço").await;
    s.board
        .move_card(&s.team, &Actor::Human, id.as_str(), "doing", None)
        .await
        .unwrap();
    let timeline = s
        .board
        .bus()
        .timeline_views(&s.team, None, 50)
        .await
        .unwrap();
    assert!(
        timeline
            .iter()
            .any(|m| m.to == "@voce" && m.body.contains("Automação em laço interrompida")),
        "{:?}",
        timeline.iter().map(|m| &m.body).collect::<Vec<_>>()
    );
    let moves = s
        .board
        .show(&s.team, id.as_str())
        .await
        .unwrap()
        .activity
        .iter()
        .filter(|a| a.action == "moved")
        .count();
    assert!(moves <= AUTOMATION_MAX_DEPTH as usize + 1, "{moves}");
}

#[tokio::test]
async fn automacao_invalida_e_recusada() {
    let s = squad().await;
    let bad = automations_from_toml(
        "[[automation]]\nwhen = \"card_enters\"\ncolumn = \"nao-existe\"\nthen = [{ add_label = \"x\" }]",
    )
    .unwrap();
    let err = s.board.set_automations(&s.team, bad).await.unwrap_err();
    assert!(err.to_string().contains("não existe"));
}

// ─────────────────────────── F06-10 ───────────────────────────

#[tokio::test]
async fn remover_coluna_com_cartoes_exige_destino() {
    let s = squad().await;
    let id = s.add("No backlog").await;
    s.board
        .move_card(&s.team, &Actor::Human, id.as_str(), "backlog", None)
        .await
        .unwrap();
    let store = s.board.bus().store();
    let board = store.get_board(&s.team).await.unwrap().unwrap();
    let columns = store.list_columns(&board.id).await.unwrap();
    let drafts: Vec<ColumnDraft> = columns
        .iter()
        .filter(|c| c.slug != "backlog")
        .map(|c| ColumnDraft {
            id: Some(c.id.clone()),
            slug: if c.slug == "doing" {
                "fazendo".into()
            } else {
                c.slug.clone()
            },
            name: c.name.clone(),
            kind: c.kind,
            wip_limit: c.wip_limit,
            wip_per_agent: c.wip_per_agent,
            requires_approval: false,
            approver_must_differ: true,
            requires_commands: vec![],
        })
        .collect();
    let err = s
        .board
        .save_columns(&s.team, drafts.clone(), vec![])
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("escolha para onde movê-los"),
        "{err}"
    );
    let saved = s
        .board
        .save_columns(&s.team, drafts, vec![("backlog".into(), "todo".into())])
        .await
        .unwrap();
    assert_eq!(saved.len(), 5);
    assert_eq!(s.column_of(&id).await, "todo");
    // A automação de Fazendo acompanhou o slug novo.
    let board = store.get_board(&s.team).await.unwrap().unwrap();
    assert!(board
        .automations
        .iter()
        .any(|a| a.column.as_deref() == Some("fazendo")));
}

// ─────────────────────────── F06-11 ───────────────────────────

async fn gated(s: &Squad, commands: Vec<String>) {
    let store = s.board.bus().store();
    let board = store.get_board(&s.team).await.unwrap().unwrap();
    let drafts = store
        .list_columns(&board.id)
        .await
        .unwrap()
        .into_iter()
        .map(|c| ColumnDraft {
            id: Some(c.id.clone()),
            requires_approval: c.slug == "done",
            requires_commands: if c.slug == "done" {
                commands.clone()
            } else {
                vec![]
            },
            slug: c.slug,
            name: c.name,
            kind: c.kind,
            wip_limit: c.wip_limit,
            wip_per_agent: c.wip_per_agent,
            approver_must_differ: true,
        })
        .collect();
    s.board.save_columns(&s.team, drafts, vec![]).await.unwrap();
}

#[tokio::test]
async fn gate_de_revisao_impede_autoaprovacao() {
    let gate = Arc::new(FakeGate::default());
    let s = squad_with(Arc::clone(&gate) as Arc<dyn Gate>).await;
    gated(&s, vec!["lint".into()]).await;
    let id = s.add("Endpoint /auth/token").await;
    let backend = s.agent("backend");
    s.board
        .move_card(&s.team, &backend, id.as_str(), "doing", None)
        .await
        .unwrap();
    s.board
        .move_card(&s.team, &backend, id.as_str(), "review", None)
        .await
        .unwrap();
    // `done` direto não passa pelo gate.
    let err = s
        .board
        .done(&s.team, &backend, id.as_str(), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "approval_required");
    let err = s
        .board
        .approve(&s.team, &backend, id.as_str(), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "self_approval");
    assert_eq!(err.hint().unwrap(), "Peça a @revisor.");
    // Rejeitar exige motivo e devolve para a coluna anterior.
    let err = s
        .board
        .reject(&s.team, &s.agent("revisor"), id.as_str(), "")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "reason_required");
    s.inbox("backend").await;
    let back = s
        .board
        .reject(
            &s.team,
            &s.agent("revisor"),
            id.as_str(),
            "o refresh não invalida o token antigo",
        )
        .await
        .unwrap();
    assert_eq!(back.card.column_slug, "doing");
    assert!(s.inbox("backend").await[0].contains("o refresh não invalida o token antigo"));
    s.board
        .move_card(&s.team, &backend, id.as_str(), "review", None)
        .await
        .unwrap();
    let approved = s
        .board
        .approve(
            &s.team,
            &s.agent("revisor"),
            id.as_str(),
            Some("testei o fluxo de refresh"),
        )
        .await
        .unwrap();
    assert_eq!(approved.card.column_slug, "done");
    assert_eq!(approved.card.card.approved_by, Some(s.id("revisor")));
    // O gate rodou na bancada do responsável.
    assert_eq!(
        gate.runs.lock().unwrap().last().unwrap(),
        &(Some(s.id("backend")), vec!["lint".to_owned()])
    );
    let actions: Vec<String> = s
        .board
        .show(&s.team, id.as_str())
        .await
        .unwrap()
        .activity
        .into_iter()
        .map(|a| a.action)
        .collect();
    assert!(actions.contains(&"rejected".into()) && actions.contains(&"approved".into()));
}

#[tokio::test]
async fn gate_com_teste_vermelho_barra_e_anexa_a_saida() {
    let s = squad_with(Arc::new(FakeGate::default())).await;
    gated(&s, vec!["lint".into(), "test".into()]).await;
    let id = s.add("Refresh token").await;
    let backend = s.agent("backend");
    s.board
        .move_card(&s.team, &backend, id.as_str(), "doing", None)
        .await
        .unwrap();
    s.board
        .move_card(&s.team, &backend, id.as_str(), "review", None)
        .await
        .unwrap();
    let err = s
        .board
        .approve(&s.team, &Actor::Human, id.as_str(), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "gate_failed");
    assert_eq!(
        err.to_string(),
        "O comando 'test' falhou (exit 1). A saída está anexada ao cartão."
    );
    let detail = s.board.show(&s.team, id.as_str()).await.unwrap();
    assert_eq!(detail.column.slug, "review");
    // O agente lê o erro sem reproduzir.
    let text = render_card(&detail, crate::time::now_ms());
    assert!(
        text.contains("assertion failed: refresh invalida o token antigo"),
        "{text}"
    );
}

// ─────────────────────────── F06-05 (leitura) ───────────────────────────

#[tokio::test]
async fn board_de_20_cartoes_cabe_em_60_linhas() {
    let s = squad().await;
    for i in 0..20 {
        let id = s
            .add(&format!("Cartão número {i} com um título razoável"))
            .await;
        let target = ["todo", "backlog", "review", "done"][i % 4];
        s.board
            .move_card(&s.team, &Actor::Human, id.as_str(), target, None)
            .await
            .unwrap();
    }
    let view = s.board.board(&s.team).await.unwrap();
    let text = render_board(&view, None, false);
    assert!(text.starts_with("QUADRO — Squad Produto"), "{text}");
    assert!(
        text.lines().count() < 60,
        "{} linhas:\n{text}",
        text.lines().count()
    );
    assert!(text.contains("A FAZER (5) · todo"));
    assert!(text.contains("sem responsável"));
}

#[tokio::test]
async fn watch_acorda_quando_algo_meu_muda() {
    let s = squad().await;
    let id = s.add("Algo").await;
    let board = s.board.clone();
    let me = s.id("backend");
    let waiter = tokio::spawn(async move { board.watch(&me, Duration::from_secs(5)).await });
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    s.board
        .update(
            &s.team,
            &Actor::Human,
            id.as_str(),
            CardPatch {
                assignee: Some("backend".into()),
                ..CardPatch::default()
            },
        )
        .await
        .unwrap();
    let event = waiter.await.unwrap().expect("evento");
    assert_eq!(event.card_id, Some(id));
    assert_eq!(event.action, "assigned");
}
