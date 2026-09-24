//! Testes de integração dos repositórios de equipe e agente.
//!
//! O mesmo contrato roda contra o SQLite e contra o `InMemoryStore` do core: se
//! os dois divergirem, os testes do domínio estariam mentindo.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use aisense_core::agent::{Agent, AgentDraft, DeliveryMode, Handle, RestartPolicy, Workbench};
use aisense_core::board::{
    ensure_board, Activity, Actor, BoardRepository, Card, CardPriority, CardQuery, CardWrite,
    ChecklistItem, Comment, WriteGuard,
};
use aisense_core::bus::{
    route, Address, BusRepository, Channel, Delivery, DeliveryState, InboxQuery, MessageKind,
    Outgoing, Sender, Target,
};
use aisense_core::repo::{
    AgentRepository, AgentSkill, InMemoryStore, RepoError, SessionRecord, SessionRepository,
    SkillRepository, TeamFilter, TeamRepository, TokenRecord, TokenRepository,
    SESSIONS_KEPT_PER_AGENT,
};
use aisense_core::skill::{parse_skill, Skill, SkillSource};
use aisense_core::team::{Team, TeamDraft};
use aisense_core::{
    ActivityId, AgentColor, AgentId, CardId, ChannelId, ColumnId, CommentId, MessageId, SessionId,
    TeamId,
};
use aisense_store::Store;

trait Repo:
    TeamRepository
    + AgentRepository
    + SessionRepository
    + SkillRepository
    + BusRepository
    + TokenRepository
    + BoardRepository
    + aisense_core::proposal::ProposalRepository
{
}
impl<T> Repo for T where
    T: TeamRepository
        + AgentRepository
        + SessionRepository
        + SkillRepository
        + BusRepository
        + TokenRepository
        + BoardRepository
        + aisense_core::proposal::ProposalRepository
{
}

fn team(name: &str, now: i64) -> Team {
    Team::create(
        &TeamDraft {
            name: name.into(),
            workdir: "/home/dev/app".into(),
            ..TeamDraft::default()
        },
        now,
    )
    .unwrap()
}

fn draft(handle: &str) -> AgentDraft {
    AgentDraft {
        handle: handle.into(),
        name: handle.to_uppercase(),
        adapter_id: "shell".into(),
        ..AgentDraft::default()
    }
}

async fn add_agent(repo: &impl Repo, team: &Team, handle: &str) -> Agent {
    let existing = repo.list_agents(&team.id).await.unwrap();
    let agent = Agent::create(team.id.clone(), &draft(handle), &existing, 1).unwrap();
    repo.create_agent(&agent).await.unwrap();
    agent
}

// ───────────────────────────── contrato ─────────────────────────────

async fn round_trips_every_field(repo: impl Repo) {
    let mut t = team("Squad Produto", 10);
    t.mission = "Entregar o checkout".into();
    t.icon = Some("rocket".into());
    t.layout = serde_json::json!({ "view": "grid", "panes": [1, 2] });
    repo.create_team(&t).await.unwrap();
    assert_eq!(repo.get_team(&t.id).await.unwrap(), Some(t.clone()));

    let mut d = draft("backend");
    d.role = "Cuida da API".into();
    d.model = Some("opus".into());
    d.workdir = Some("/srv/api".into());
    d.env = BTreeMap::from([("RUST_LOG".into(), "debug".into())]);
    d.args = vec!["--verbose".into(), "--dir=a b".into()];
    d.color = Some(AgentColor::Teal);
    d.autostart = true;
    d.restart_policy = RestartPolicy::Always;
    d.delivery_mode = DeliveryMode::Push;
    d.workbench = Workbench::Own;
    let a = Agent::create(t.id.clone(), &d, &[], 20).unwrap();
    repo.create_agent(&a).await.unwrap();

    assert_eq!(repo.get_agent(&a.id).await.unwrap(), Some(a.clone()));
    let by_handle = repo
        .find_agent_by_handle(&t.id, &Handle::parse("backend").unwrap())
        .await
        .unwrap();
    assert_eq!(by_handle, Some(a));
}

async fn deleting_a_team_cascades(repo: impl Repo) {
    let (a, b) = (team("A", 1), team("B", 2));
    repo.create_team(&a).await.unwrap();
    repo.create_team(&b).await.unwrap();
    let doomed = add_agent(&repo, &a, "backend").await;
    add_agent(&repo, &a, "frontend").await;
    add_agent(&repo, &b, "backend").await;

    repo.delete_team(&a.id).await.unwrap();
    assert_eq!(repo.get_team(&a.id).await.unwrap(), None);
    assert!(repo.list_agents(&a.id).await.unwrap().is_empty());
    assert_eq!(repo.get_agent(&doomed.id).await.unwrap(), None);
    assert_eq!(
        repo.list_agents(&b.id).await.unwrap().len(),
        1,
        "other teams untouched"
    );
    assert!(matches!(
        repo.delete_team(&a.id).await,
        Err(RepoError::TeamNotFound(_))
    ));
}

async fn handles_are_unique_per_team(repo: impl Repo) {
    let (a, b) = (team("A", 1), team("B", 2));
    repo.create_team(&a).await.unwrap();
    repo.create_team(&b).await.unwrap();
    add_agent(&repo, &a, "backend").await;

    // Contorna a validação do core (lista vazia) para provar que a porta também garante.
    let dup = Agent::create(a.id.clone(), &draft("backend"), &[], 2).unwrap();
    let err = repo.create_agent(&dup).await.unwrap_err();
    assert!(
        matches!(err, RepoError::DuplicateHandle(ref h) if h == "backend"),
        "{err:?}"
    );

    // O mesmo handle em outra equipe é outro endereço.
    add_agent(&repo, &b, "backend").await;

    // Renomear para um handle ocupado também é recusado.
    let mut front = add_agent(&repo, &a, "frontend").await;
    front.handle = Handle::parse("backend").unwrap();
    assert!(matches!(
        repo.update_agent(&front).await,
        Err(RepoError::DuplicateHandle(_))
    ));
}

async fn agents_need_an_existing_team(repo: impl Repo) {
    let ghost = team("ghost", 1);
    let orphan = Agent::create(ghost.id.clone(), &draft("backend"), &[], 1).unwrap();
    assert!(matches!(
        repo.create_agent(&orphan).await,
        Err(RepoError::TeamNotFound(_))
    ));
}

async fn archiving_hides_but_keeps(repo: impl Repo) {
    let (old, new) = (team("Antiga", 1), team("Nova", 2));
    repo.create_team(&old).await.unwrap();
    repo.create_team(&new).await.unwrap();
    add_agent(&repo, &old, "backend").await;

    repo.set_team_archived(&old.id, Some(50)).await.unwrap();
    let visible = repo.list_teams(TeamFilter::default()).await.unwrap();
    assert_eq!(
        visible.iter().map(|t| &t.name).collect::<Vec<_>>(),
        ["Nova"]
    );
    let all = repo
        .list_teams(TeamFilter {
            include_archived: true,
        })
        .await
        .unwrap();
    assert_eq!(
        all.iter().map(|t| &t.name).collect::<Vec<_>>(),
        ["Antiga", "Nova"]
    );
    assert_eq!(all[0].archived_at, Some(50));
    assert_eq!(repo.list_agents(&old.id).await.unwrap().len(), 1);

    repo.set_team_archived(&old.id, None).await.unwrap();
    assert_eq!(
        repo.list_teams(TeamFilter::default()).await.unwrap().len(),
        2
    );
}

async fn updates_and_orders(repo: impl Repo) {
    let mut t = team("A", 1);
    repo.create_team(&t).await.unwrap();
    let mut first = add_agent(&repo, &t, "backend").await;
    let second = add_agent(&repo, &t, "frontend").await;
    assert_eq!((first.position, second.position), (0, 1));

    first.position = 5;
    let mut d = first.to_draft();
    d.name = "Backend Sênior".into();
    first.apply(&d, &[], 99).unwrap();
    repo.update_agent(&first).await.unwrap();
    let listed = repo.list_agents(&t.id).await.unwrap();
    assert_eq!(
        listed.iter().map(|a| a.handle.as_str()).collect::<Vec<_>>(),
        ["frontend", "backend"]
    );
    assert_eq!(listed[1].name, "Backend Sênior");

    let mut td = t.to_draft();
    td.name = "Renomeada".into();
    t.apply(&td, 100).unwrap();
    repo.update_team(&t).await.unwrap();
    assert_eq!(
        repo.get_team(&t.id).await.unwrap().unwrap().name,
        "Renomeada"
    );

    repo.delete_agent(&second.id).await.unwrap();
    assert_eq!(repo.list_agents(&t.id).await.unwrap().len(), 1);
}

async fn missing_entities_are_reported(repo: impl Repo) {
    let t = team("fantasma", 1);
    assert!(matches!(
        repo.update_team(&t).await,
        Err(RepoError::TeamNotFound(_))
    ));
    assert!(matches!(
        repo.set_team_archived(&TeamId::new(), Some(1)).await,
        Err(RepoError::TeamNotFound(_))
    ));
    assert!(matches!(
        repo.delete_agent(&AgentId::new()).await,
        Err(RepoError::AgentNotFound(_))
    ));
    let a = Agent::create(t.id.clone(), &draft("backend"), &[], 1).unwrap();
    assert!(matches!(
        repo.update_agent(&a).await,
        Err(RepoError::AgentNotFound(_))
    ));
    assert_eq!(repo.get_agent(&a.id).await.unwrap(), None);
}

fn session(agent: &Agent, started_at: i64) -> SessionRecord {
    SessionRecord {
        id: SessionId::new(),
        agent_id: agent.id.clone(),
        pid: Some(4242),
        started_at,
        ended_at: None,
        exit_code: None,
        log_path: "/tmp/agente.log".into(),
        log_offset: u64::try_from(started_at * 100).ok(),
    }
}

async fn sessions_record_start_and_end(repo: impl Repo) {
    let t = team("A", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "backend").await;

    let first = session(&a, 10);
    let second = session(&a, 20);
    repo.start_session(&first).await.unwrap();
    repo.start_session(&second).await.unwrap();
    repo.end_session(&first.id, 15, Some(3)).await.unwrap();

    let listed = repo.list_sessions(&a.id, 10).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0], second, "mais recente primeiro");
    assert_eq!(listed[1].ended_at, Some(15));
    assert_eq!(listed[1].exit_code, Some(3));
    assert_eq!(listed[1].pid, Some(4242));
    assert_eq!(
        listed[1].log_offset,
        Some(1000),
        "onde a sessão começa no log"
    );

    // Sessão podada ou inexistente: fim silencioso, não erro.
    repo.end_session(&SessionId::new(), 1, None).await.unwrap();
    assert!(matches!(
        repo.start_session(&first).await,
        Err(RepoError::AlreadyExists(_))
    ));
}

async fn sessions_need_an_agent_and_follow_it(repo: impl Repo) {
    let t = team("A", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "backend").await;
    let ghost = Agent::create(t.id.clone(), &draft("fantasma"), &[], 1).unwrap();
    assert!(matches!(
        repo.start_session(&session(&ghost, 1)).await,
        Err(RepoError::AgentNotFound(_))
    ));

    repo.start_session(&session(&a, 1)).await.unwrap();
    repo.delete_agent(&a.id).await.unwrap();
    assert!(repo.list_sessions(&a.id, 10).await.unwrap().is_empty());
}

async fn sessions_are_pruned_per_agent(repo: impl Repo) {
    let t = team("A", 1);
    repo.create_team(&t).await.unwrap();
    let (a, b) = (
        add_agent(&repo, &t, "alfa").await,
        add_agent(&repo, &t, "beta").await,
    );
    repo.start_session(&session(&b, 0)).await.unwrap();
    let total = SESSIONS_KEPT_PER_AGENT + 5;
    for i in 0..total {
        repo.start_session(&session(&a, i64::try_from(i).unwrap() + 1))
            .await
            .unwrap();
    }
    let kept = repo.list_sessions(&a.id, total).await.unwrap();
    assert_eq!(kept.len(), SESSIONS_KEPT_PER_AGENT);
    assert_eq!(
        kept[0].started_at,
        i64::try_from(total).unwrap(),
        "fica o mais novo"
    );
    assert_eq!(
        repo.list_sessions(&b.id, 10).await.unwrap().len(),
        1,
        "outro agente intacto"
    );
}

fn skill(name: &str, description: &str, source: SkillSource) -> Skill {
    let md =
        format!("---\nname: {name}\ndescription: {description}\ntargets: [claude]\n---\ncorpo\n");
    parse_skill(&md, "t/SKILL.md", source).unwrap()
}

async fn skills_sync_by_slug_without_losing_assignments(repo: impl Repo) {
    let t = team("A", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "backend").await;

    let first = repo
        .sync_skills(
            &[
                skill("revisor", "Revisa.", SkillSource::Builtin),
                skill("docs", "Documenta.", SkillSource::Builtin),
            ],
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        first.iter().map(|r| r.slug.as_str()).collect::<Vec<_>>(),
        ["docs", "revisor"]
    );
    let revisor = first.iter().find(|r| r.slug == "revisor").unwrap().clone();
    assert_eq!(
        (revisor.source.as_str(), revisor.path.as_str()),
        ("builtin", "builtin:revisor")
    );
    assert_eq!(revisor.targets, ["claude"]);

    repo.set_agent_skills(
        &a.id,
        &[AgentSkill {
            skill_id: revisor.id.clone(),
            enabled: true,
        }],
    )
    .await
    .unwrap();

    // Editou no disco (agora do usuário, outra descrição) e "docs" sumiu (SKILL.md quebrado).
    let user = SkillSource::User {
        dir: "/u/revisor".into(),
    };
    let second = repo
        .sync_skills(&[skill("revisor", "Revisa com rigor.", user)], 20)
        .await
        .unwrap();
    let updated = second.iter().find(|r| r.slug == "revisor").unwrap();
    assert_eq!(updated.id, revisor.id, "mesma identidade");
    assert_eq!(updated.description, "Revisa com rigor.");
    assert_eq!(updated.source, "user");
    assert_eq!((updated.created_at, updated.updated_at), (10, 20));
    assert_eq!(second.len(), 2, "docs fica no banco mesmo fora do disco");
    assert_eq!(
        repo.agent_skills(&a.id).await.unwrap().len(),
        1,
        "atribuição sobrevive"
    );

    // Sem mudança, `updated_at` não mexe.
    let third = repo
        .sync_skills(
            &[skill(
                "revisor",
                "Revisa com rigor.",
                SkillSource::User {
                    dir: "/u/revisor".into(),
                },
            )],
            30,
        )
        .await
        .unwrap();
    assert_eq!(
        third
            .iter()
            .find(|r| r.slug == "revisor")
            .unwrap()
            .updated_at,
        20
    );
    assert_eq!(repo.list_skills().await.unwrap(), third);
}

async fn agent_skills_keep_order_and_follow_the_agent(repo: impl Repo) {
    let t = team("A", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "backend").await;
    let b = add_agent(&repo, &t, "frontend").await;
    let records = repo
        .sync_skills(
            &[
                skill("um", "Um.", SkillSource::Builtin),
                skill("dois", "Dois.", SkillSource::Builtin),
                skill("tres", "Três.", SkillSource::Builtin),
            ],
            1,
        )
        .await
        .unwrap();
    let id = |slug: &str| records.iter().find(|r| r.slug == slug).unwrap().id.clone();
    let entry = |slug: &str, enabled| AgentSkill {
        skill_id: id(slug),
        enabled,
    };

    repo.set_agent_skills(
        &a.id,
        &[
            entry("tres", true),
            entry("um", false),
            entry("tres", false),
            entry("dois", true),
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        repo.agent_skills(&a.id).await.unwrap(),
        [entry("tres", true), entry("um", false), entry("dois", true)],
        "ordem dada, repetida vale a primeira"
    );
    repo.set_agent_skills(&b.id, &[entry("um", true)])
        .await
        .unwrap();
    assert_eq!(repo.skill_users(&id("um")).await.unwrap().len(), 2);

    // Trocar a lista substitui tudo.
    repo.set_agent_skills(&a.id, &[entry("dois", true)])
        .await
        .unwrap();
    assert_eq!(
        repo.agent_skills(&a.id).await.unwrap(),
        [entry("dois", true)]
    );

    // Erros não mudam nada.
    let ghost = aisense_core::SkillId::new();
    assert!(matches!(
        repo.set_agent_skills(
            &a.id,
            &[
                entry("um", true),
                AgentSkill {
                    skill_id: ghost,
                    enabled: true
                }
            ]
        )
        .await,
        Err(RepoError::SkillNotFound(_))
    ));
    assert_eq!(
        repo.agent_skills(&a.id).await.unwrap(),
        [entry("dois", true)]
    );
    assert!(matches!(
        repo.set_agent_skills(&AgentId::new(), &[]).await,
        Err(RepoError::AgentNotFound(_))
    ));

    // Excluir o agente leva as atribuições junto.
    repo.delete_agent(&b.id).await.unwrap();
    assert_eq!(
        repo.skill_users(&id("um")).await.unwrap(),
        Vec::<AgentId>::new()
    );
    repo.delete_team(&t.id).await.unwrap();
    assert!(repo.agent_skills(&a.id).await.unwrap().is_empty());
}

async fn bus_round_trips_messages_and_deliveries(repo: impl Repo) {
    let t = team("Squad", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "backend").await;
    let b = add_agent(&repo, &t, "frontend").await;
    let c = add_agent(&repo, &t, "revisor").await;
    let running = |_: &AgentId| true;
    let from = |agent: &Agent| Sender::Agent {
        agent_id: agent.id.clone(),
    };

    // DM com todos os campos.
    let mut out = Outgoing::message(
        from(&a),
        Address::parse("@frontend").unwrap(),
        "oi\nlinha 2",
    );
    out.subject = Some("contrato".into());
    out.meta.timeout_s = Some(30);
    out.meta.attachments = vec!["/tmp/a.md".into()];
    out.kind = MessageKind::Request;
    let dm = route(&repo, &t.id, out, 10, running).await.unwrap();
    assert_eq!(
        repo.get_message(&dm.message.id).await.unwrap(),
        Some(dm.message.clone())
    );
    assert_eq!(
        repo.deliveries_of(&dm.message.id).await.unwrap(),
        dm.deliveries
    );

    // Os quatro destinos voltam iguais.
    let channel = route(
        &repo,
        &t.id,
        Outgoing::message(from(&b), Address::parse("#geral").unwrap(), "c"),
        11,
        running,
    )
    .await
    .unwrap();
    let all = route(
        &repo,
        &t.id,
        Outgoing::message(Sender::Human, Address::All, "todos"),
        12,
        running,
    )
    .await
    .unwrap();
    let human = route(
        &repo,
        &t.id,
        Outgoing::message(Sender::System, Address::Human, "aviso"),
        13,
        running,
    )
    .await
    .unwrap();
    let mut reply = Outgoing::message(from(&b), Address::parse("@backend").unwrap(), "resposta");
    reply.kind = MessageKind::Response;
    reply.reply_to = Some(dm.message.id.clone());
    let answer = route(&repo, &t.id, reply, 14, running).await.unwrap();
    for routed in [&channel, &all, &human, &answer] {
        assert_eq!(
            repo.get_message(&routed.message.id).await.unwrap().as_ref(),
            Some(&routed.message)
        );
    }
    assert!(matches!(channel.message.to, Target::Channel { .. }));
    assert_eq!(repo.list_channels(&t.id).await.unwrap().len(), 1);
    assert_eq!(
        repo.replies_to(&dm.message.id).await.unwrap(),
        vec![answer.message.clone()]
    );

    // Caixa de entrada de @frontend: DM, broadcast (o canal foi dele).
    let unread = InboxQuery {
        unread_only: true,
        after: None,
        limit: 50,
    };
    let inbox = repo.inbox(&b.id, &unread).await.unwrap();
    let ids: Vec<_> = inbox.iter().map(|i| i.message.id.clone()).collect();
    assert_eq!(ids, vec![dm.message.id.clone(), all.message.id.clone()]);
    let after = InboxQuery {
        after: Some(dm.message.id.clone()),
        ..unread.clone()
    };
    assert_eq!(repo.inbox(&b.id, &after).await.unwrap().len(), 1);

    // Entrega e leitura.
    let mut delivery = inbox[0].delivery.clone();
    delivery.state = DeliveryState::Delivered;
    delivery.delivered_at = Some(20);
    delivery.attempts = 1;
    repo.update_delivery(&delivery).await.unwrap();
    assert_eq!(
        repo.deliveries_of(&dm.message.id).await.unwrap(),
        vec![delivery.clone()]
    );
    assert_eq!(repo.unread_counts(&t.id).await.unwrap(), {
        // @backend: canal, broadcast e a resposta.
        let mut v = vec![(a.id.clone(), 3), (b.id.clone(), 2), (c.id.clone(), 2)];
        v.sort_by(|x, y| x.0.as_str().cmp(y.0.as_str()));
        v
    });
    assert_eq!(
        repo.mark_read(&b.id, &[dm.message.id.clone(), all.message.id.clone()], 30)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        repo.mark_read(&b.id, std::slice::from_ref(&dm.message.id), 31)
            .await
            .unwrap(),
        0
    );
    assert!(repo.inbox(&b.id, &unread).await.unwrap().is_empty());
    let every = InboxQuery {
        unread_only: false,
        ..unread
    };
    assert_eq!(
        repo.inbox(&b.id, &every).await.unwrap()[0].delivery.read_at,
        Some(30)
    );

    // Linha do tempo: mais nova primeiro, com cursor.
    let timeline = repo.timeline(&t.id, None, 2).await.unwrap();
    assert_eq!(
        timeline,
        vec![answer.message.clone(), human.message.clone()]
    );
    let older = repo
        .timeline(&t.id, Some(&human.message.id), 10)
        .await
        .unwrap();
    assert_eq!(older.len(), 3);

    let missing = Delivery::pending(&MessageId::new(), &b.id);
    assert!(repo.update_delivery(&missing).await.is_err());
}

async fn bus_channels_are_unique_and_retention_prunes(repo: impl Repo) {
    let t = team("Squad", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "backend").await;
    let _b = add_agent(&repo, &t, "frontend").await;
    let ch = Channel {
        id: ChannelId::new(),
        team_id: t.id.clone(),
        slug: "geral".into(),
        topic: String::new(),
        created_at: 1,
    };
    repo.create_channel(&ch).await.unwrap();
    let dup = Channel {
        id: ChannelId::new(),
        ..ch.clone()
    };
    assert!(matches!(
        repo.create_channel(&dup).await,
        Err(RepoError::AlreadyExists(_))
    ));
    assert_eq!(
        repo.channel_by_slug(&t.id, "geral").await.unwrap(),
        Some(ch)
    );

    let from = Sender::Agent {
        agent_id: a.id.clone(),
    };
    for (i, at) in [100, 200, 300].into_iter().enumerate() {
        route(
            &repo,
            &t.id,
            Outgoing::message(from.clone(), Address::All, format!("m{i}")),
            at,
            |_| true,
        )
        .await
        .unwrap();
    }
    assert_eq!(repo.prune_messages(250).await.unwrap(), 2);
    let left = repo.timeline(&t.id, None, 10).await.unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].body, "m2");

    // Excluir o agente leva as entregas dele; a mensagem que ele mandou fica.
    repo.delete_agent(&a.id).await.unwrap();
    assert_eq!(repo.timeline(&t.id, None, 10).await.unwrap().len(), 1);
    repo.delete_team(&t.id).await.unwrap();
    assert!(repo.timeline(&t.id, None, 10).await.unwrap().is_empty());
    assert!(repo.list_channels(&t.id).await.unwrap().is_empty());
}

async fn tokens_live_with_the_session(repo: impl Repo) {
    let t = team("A", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "backend").await;
    let s1 = session(&a, 10);
    let s2 = session(&a, 20);
    repo.start_session(&s1).await.unwrap();
    repo.start_session(&s2).await.unwrap();
    let record = |s: &SessionRecord, expires_at| TokenRecord {
        agent_id: a.id.clone(),
        session_id: s.id.clone(),
        expires_at,
    };
    repo.insert_token("tok-1", &record(&s1, 1_000))
        .await
        .unwrap();
    repo.insert_token("tok-2", &record(&s2, 1_000))
        .await
        .unwrap();
    assert!(repo
        .insert_token("tok-1", &record(&s2, 1_000))
        .await
        .is_err());

    assert_eq!(
        repo.find_token("tok-1", 500).await.unwrap(),
        Some(record(&s1, 1_000))
    );
    assert_eq!(
        repo.find_token("tok-1", 1_000).await.unwrap(),
        None,
        "expirado"
    );
    assert_eq!(repo.find_token("nada", 500).await.unwrap(), None);

    // Fim da sessão: revogado na hora; o da outra sessão continua.
    assert_eq!(repo.revoke_session_tokens(&s1.id).await.unwrap(), 1);
    assert_eq!(repo.find_token("tok-1", 500).await.unwrap(), None);
    assert!(repo.find_token("tok-2", 500).await.unwrap().is_some());
    assert_eq!(repo.revoke_all_tokens().await.unwrap(), 1);
    assert_eq!(repo.find_token("tok-2", 500).await.unwrap(), None);

    // Agente excluído leva os tokens junto.
    let s3 = session(&a, 30);
    repo.start_session(&s3).await.unwrap();
    repo.insert_token("tok-3", &record(&s3, 1_000))
        .await
        .unwrap();
    repo.delete_agent(&a.id).await.unwrap();
    assert_eq!(repo.find_token("tok-3", 500).await.unwrap(), None);
}

// ───────────────────────────── quadro (F06-02) ─────────────────────────────

fn card(team: &Team, column: &ColumnId, title: &str, position: i64) -> Card {
    Card {
        id: CardId::new(),
        team_id: team.id.clone(),
        column_id: column.clone(),
        title: title.into(),
        body: "corpo".into(),
        assignee: None,
        created_by: None,
        parent_id: None,
        position,
        priority: CardPriority::Normal,
        labels: vec![],
        checklist: vec![],
        links: vec![],
        block_reason: None,
        version: 1,
        archived_at: None,
        approved_by: None,
        approved_at: None,
        column_since: 5,
        created_at: 5,
        updated_at: 5,
    }
}

fn guard(version: i64) -> WriteGuard {
    WriteGuard {
        expected_version: version,
        ..WriteGuard::default()
    }
}

async fn board_round_trips_and_guards_writes(repo: impl Repo) {
    let t = team("Squad", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "backend").await;
    let b = add_agent(&repo, &t, "revisor").await;
    let board = ensure_board(&repo, &t.id, 2).await.unwrap();
    assert!(matches!(
        repo.create_board(&board, &[]).await,
        Err(RepoError::AlreadyExists(_))
    ));
    assert_eq!(repo.get_board(&t.id).await.unwrap(), Some(board.clone()));
    let columns = repo.list_columns(&board.id).await.unwrap();
    assert_eq!(columns.len(), 6);
    let (todo, doing) = (columns[1].id.clone(), columns[2].id.clone());

    let mut c1 = card(&t, &todo, "Migrar /users", 0);
    c1.labels = vec!["backend".into()];
    c1.checklist = vec![ChecklistItem {
        text: "teste".into(),
        done: true,
    }];
    c1.priority = CardPriority::High;
    c1.created_by = Some(a.id.clone());
    repo.insert_card(&c1).await.unwrap();
    assert_eq!(repo.get_card(&c1.id).await.unwrap(), Some(c1.clone()));
    assert!(matches!(
        repo.insert_card(&c1).await,
        Err(RepoError::AlreadyExists(_))
    ));
    let c2 = card(&t, &doing, "Middleware", 0);
    repo.insert_card(&c2).await.unwrap();

    // Consultas por coluna, responsável, label e estado.
    let all = repo.list_cards(&t.id, &CardQuery::default()).await.unwrap();
    assert_eq!(
        all.iter().map(|c| &c.id).collect::<Vec<_>>(),
        [&c1.id, &c2.id]
    );
    let by_column = CardQuery {
        column_id: Some(doing.clone()),
        ..CardQuery::default()
    };
    assert_eq!(
        repo.list_cards(&t.id, &by_column).await.unwrap(),
        std::slice::from_ref(&c2)
    );
    let by_label = CardQuery {
        label: Some("backend".into()),
        ..CardQuery::default()
    };
    assert_eq!(
        repo.list_cards(&t.id, &by_label).await.unwrap(),
        [c1.clone()]
    );

    // Claim: versão e "ninguém pegou" conferidos na gravação.
    let mut claimed = c1.clone();
    claimed.assignee = Some(a.id.clone());
    claimed.version = 2;
    let claim = WriteGuard {
        require_unassigned: true,
        ..guard(1)
    };
    assert_eq!(
        repo.update_card(&claimed, &claim).await.unwrap(),
        CardWrite::Written
    );
    let mut late = c1.clone();
    late.assignee = Some(b.id.clone());
    late.version = 2;
    assert_eq!(
        repo.update_card(&late, &claim).await.unwrap(),
        CardWrite::Stale
    );
    let again = WriteGuard {
        require_unassigned: true,
        ..guard(2)
    };
    late.version = 3;
    assert_eq!(
        repo.update_card(&late, &again).await.unwrap(),
        CardWrite::Stale
    );
    let mine = CardQuery {
        assignee: Some(a.id.clone()),
        ..CardQuery::default()
    };
    assert_eq!(
        repo.list_cards(&t.id, &mine).await.unwrap(),
        [claimed.clone()]
    );
    let free = CardQuery {
        unassigned: true,
        ..CardQuery::default()
    };
    assert_eq!(
        repo.list_cards(&t.id, &free).await.unwrap(),
        std::slice::from_ref(&c2)
    );

    // WIP por agente e total, contando os outros cartões da coluna de destino.
    let mut c2_mine = c2.clone();
    c2_mine.assignee = Some(a.id.clone());
    c2_mine.version = 2;
    repo.update_card(&c2_mine, &guard(1)).await.unwrap();
    let mut into_doing = claimed.clone();
    into_doing.column_id = doing.clone();
    into_doing.version = 3;
    let per_agent = WriteGuard {
        wip_per_agent: Some(1),
        ..guard(2)
    };
    assert_eq!(
        repo.update_card(&into_doing, &per_agent).await.unwrap(),
        CardWrite::WipFull
    );
    let total = WriteGuard {
        wip_limit: Some(2),
        ..guard(2)
    };
    assert_eq!(
        repo.update_card(&into_doing, &total).await.unwrap(),
        CardWrite::Written
    );
    let total_full = WriteGuard {
        wip_limit: Some(1),
        ..guard(2)
    };
    let mut c2_again = c2_mine.clone();
    c2_again.version = 3;
    c2_again.title = "renomeado".into();
    assert_eq!(
        repo.update_card(&c2_again, &total_full).await.unwrap(),
        CardWrite::WipFull
    );

    // Dependências.
    repo.add_dependency(&c2.id, &c1.id).await.unwrap();
    repo.add_dependency(&c2.id, &c1.id).await.unwrap();
    assert_eq!(
        repo.dependencies(&t.id).await.unwrap(),
        [(c2.id.clone(), c1.id.clone())]
    );
    assert!(repo.add_dependency(&c1.id, &c1.id).await.is_err());
    repo.remove_dependency(&c2.id, &c1.id).await.unwrap();
    assert!(repo.dependencies(&t.id).await.unwrap().is_empty());

    // Comentários e histórico com autor (agente, humano, sistema).
    for (i, author) in [
        Actor::Agent {
            agent_id: b.id.clone(),
        },
        Actor::Human,
        Actor::System,
    ]
    .into_iter()
    .enumerate()
    {
        let at = 10 + i64::try_from(i).unwrap();
        repo.insert_comment(&Comment {
            id: CommentId::new(),
            card_id: c1.id.clone(),
            author: author.clone(),
            body: format!("comentário {i}"),
            created_at: at,
        })
        .await
        .unwrap();
        repo.insert_activity(&Activity {
            id: ActivityId::new(),
            card_id: c1.id.clone(),
            actor: author,
            action: "commented".into(),
            detail: serde_json::json!({ "n": i }),
            created_at: at,
        })
        .await
        .unwrap();
    }
    let comments = repo.list_comments(&c1.id).await.unwrap();
    assert_eq!(comments.len(), 3);
    assert_eq!(comments[1].author, Actor::Human);
    assert_eq!(
        repo.comment_counts(&t.id).await.unwrap(),
        [(c1.id.clone(), 3)]
    );
    let history = repo.list_activity(&c1.id).await.unwrap();
    assert_eq!(history[0].detail["n"], 0);
    assert_eq!(repo.activity_since(&t.id, 10).await.unwrap().len(), 2);

    // Agente removido: cartão fica sem responsável, a autoria vira "sistema".
    repo.delete_agent(&b.id).await.unwrap();
    assert_eq!(
        repo.list_comments(&c1.id).await.unwrap()[0].author,
        Actor::System
    );
    repo.delete_agent(&a.id).await.unwrap();
    assert_eq!(repo.get_card(&c1.id).await.unwrap().unwrap().assignee, None);

    // Arquivado some da listagem padrão, mas não do banco.
    let mut archived = repo.get_card(&c2.id).await.unwrap().unwrap();
    archived.archived_at = Some(99);
    archived.version += 1;
    repo.update_card(&archived, &guard(archived.version - 1))
        .await
        .unwrap();
    assert_eq!(
        repo.list_cards(&t.id, &CardQuery::default())
            .await
            .unwrap()
            .len(),
        1
    );
    let with_archived = CardQuery {
        include_archived: true,
        ..CardQuery::default()
    };
    assert_eq!(
        repo.list_cards(&t.id, &with_archived).await.unwrap().len(),
        2
    );

    // Apagar a equipe leva o quadro junto.
    repo.delete_team(&t.id).await.unwrap();
    assert_eq!(repo.get_board(&t.id).await.unwrap(), None);
    assert_eq!(repo.get_card(&c1.id).await.unwrap(), None);
}

async fn board_columns_are_replaced_atomically(repo: impl Repo) {
    let t = team("Squad", 1);
    repo.create_team(&t).await.unwrap();
    let board = ensure_board(&repo, &t.id, 2).await.unwrap();
    let mut columns = repo.list_columns(&board.id).await.unwrap();
    let backlog = columns[0].id.clone();
    let c = card(&t, &backlog, "no backlog", 0);
    repo.insert_card(&c).await.unwrap();

    // Trocar dois slugs de lugar e remover o backlog, movendo o cartão.
    columns.remove(0);
    columns[0].slug = "doing".into();
    columns[1].slug = "todo".into();
    columns.reverse();
    for (i, col) in columns.iter_mut().enumerate() {
        col.position = u32::try_from(i).unwrap();
    }
    let target = columns[0].id.clone();
    // Sem destino para o cartão: recusado, nada muda.
    assert!(repo
        .replace_columns(&board.id, &columns, &[], 3)
        .await
        .is_err());
    assert_eq!(repo.list_columns(&board.id).await.unwrap().len(), 6);
    repo.replace_columns(&board.id, &columns, &[(backlog, target.clone())], 3)
        .await
        .unwrap();
    let saved = repo.list_columns(&board.id).await.unwrap();
    assert_eq!(saved, columns);
    let moved = repo.get_card(&c.id).await.unwrap().unwrap();
    assert_eq!(
        (moved.column_id, moved.column_since, moved.version),
        (target, 3, 2)
    );
}

async fn channel_members_follow_channel_and_agent(repo: impl Repo) {
    let t = team("Squad", 1);
    repo.create_team(&t).await.unwrap();
    let other = team("Outra", 1);
    repo.create_team(&other).await.unwrap();
    let a = add_agent(&repo, &t, "p1").await;
    let b = add_agent(&repo, &t, "p2").await;
    let stranger = add_agent(&repo, &other, "estranho").await;
    let ch = Channel {
        id: ChannelId::new(),
        team_id: t.id.clone(),
        slug: "pesquisa".into(),
        topic: String::new(),
        created_at: 1,
    };
    repo.create_channel(&ch).await.unwrap();
    assert!(repo.channel_members(&ch.id).await.unwrap().is_empty());
    repo.set_channel_members(&ch.id, &[a.id.clone(), b.id.clone()])
        .await
        .unwrap();
    assert_eq!(repo.channel_members(&ch.id).await.unwrap().len(), 2);
    // Agente de outra equipe não entra, e nada muda.
    assert!(matches!(
        repo.set_channel_members(&ch.id, std::slice::from_ref(&stranger.id))
            .await,
        Err(RepoError::AgentNotFound(_))
    ));
    repo.set_channel_topic(&ch.id, "achados").await.unwrap();
    assert_eq!(
        repo.channel_by_slug(&t.id, "pesquisa")
            .await
            .unwrap()
            .unwrap()
            .topic,
        "achados"
    );
    repo.delete_agent(&a.id).await.unwrap();
    assert_eq!(
        repo.channel_members(&ch.id).await.unwrap(),
        std::slice::from_ref(&b.id)
    );
    repo.delete_channel(&ch.id).await.unwrap();
    assert!(repo
        .channel_by_slug(&t.id, "pesquisa")
        .await
        .unwrap()
        .is_none());
    assert!(repo.channel_members(&ch.id).await.unwrap().is_empty());
}

async fn proposals_are_decided_once(repo: impl Repo) {
    use aisense_core::proposal::{Proposal, ProposalAction, ProposalState};
    let t = team("Squad", 1);
    repo.create_team(&t).await.unwrap();
    let a = add_agent(&repo, &t, "coordenador").await;
    let p = Proposal {
        id: aisense_core::ProposalId::new(),
        team_id: t.id.clone(),
        proposed_by: Some(a.id.clone()),
        action: ProposalAction::ChangeColumns {
            change: "coluna QA".into(),
        },
        reason: "falta QA".into(),
        state: ProposalState::Pending,
        created_at: 5,
        decided_at: None,
        decision_note: None,
    };
    repo.insert_proposal(&p).await.unwrap();
    assert_eq!(repo.get_proposal(&p.id).await.unwrap(), Some(p.clone()));
    assert_eq!(repo.list_proposals(&t.id, true).await.unwrap().len(), 1);
    assert!(repo
        .decide_proposal(&p.id, ProposalState::Rejected, 9, Some("não"))
        .await
        .unwrap());
    assert!(!repo
        .decide_proposal(&p.id, ProposalState::Accepted, 10, None)
        .await
        .unwrap());
    let got = repo.get_proposal(&p.id).await.unwrap().unwrap();
    assert_eq!(
        (got.state, got.decided_at, got.decision_note.as_deref()),
        (ProposalState::Rejected, Some(9), Some("não"))
    );
    assert!(repo.list_proposals(&t.id, true).await.unwrap().is_empty());
    repo.delete_agent(&a.id).await.unwrap();
    assert_eq!(
        repo.get_proposal(&p.id).await.unwrap().unwrap().proposed_by,
        None
    );
}

macro_rules! contract {
    ($($name:ident),+ $(,)?) => {
        mod sqlite {
            use super::*;
            $(
                #[tokio::test]
                async fn $name() {
                    super::$name(Store::open_in_memory().await.unwrap()).await;
                }
            )+
        }
        mod in_memory {
            use super::*;
            $(
                #[tokio::test]
                async fn $name() {
                    super::$name(InMemoryStore::new()).await;
                }
            )+
        }
    };
}

contract!(
    round_trips_every_field,
    deleting_a_team_cascades,
    handles_are_unique_per_team,
    agents_need_an_existing_team,
    archiving_hides_but_keeps,
    updates_and_orders,
    missing_entities_are_reported,
    sessions_record_start_and_end,
    sessions_need_an_agent_and_follow_it,
    sessions_are_pruned_per_agent,
    skills_sync_by_slug_without_losing_assignments,
    agent_skills_keep_order_and_follow_the_agent,
    bus_round_trips_messages_and_deliveries,
    bus_channels_are_unique_and_retention_prunes,
    tokens_live_with_the_session,
    board_round_trips_and_guards_writes,
    channel_members_follow_channel_and_agent,
    proposals_are_decided_once,
    board_columns_are_replaced_atomically,
);

#[tokio::test]
async fn timeline_of_100k_messages_answers_in_under_20ms() {
    // Aceite da F05-02. Inserção em lote por SQL (o que se mede é a consulta).
    let store = Store::open_in_memory().await.unwrap();
    let t = team("Grande", 1);
    store.create_team(&t).await.unwrap();
    let other = team("Outra", 1);
    store.create_team(&other).await.unwrap();
    let a = add_agent(&store, &t, "backend").await;
    let mut tx = store.pool().begin().await.unwrap();
    for i in 0..100_000u32 {
        let team_id = if i % 10 == 0 {
            other.id.as_str()
        } else {
            t.id.as_str()
        };
        sqlx::query(
            "INSERT INTO messages (id, team_id, kind, from_kind, from_agent, broadcast, body, created_at) \
             VALUES (?, ?, 'message', 'agent', ?, 1, 'corpo da mensagem', ?)",
        )
        .bind(MessageId::new().as_str())
        .bind(team_id)
        .bind(a.id.as_str())
        .bind(i64::from(i))
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();

    let first = store.timeline(&t.id, None, 100).await.unwrap();
    assert_eq!(first.len(), 100);
    let started = std::time::Instant::now();
    let page = store
        .timeline(&t.id, Some(&first[99].id), 100)
        .await
        .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(page.len(), 100);
    assert!(page.iter().all(|m| m.team_id == t.id));
    assert!(elapsed.as_millis() < 20, "timeline levou {elapsed:?}");
}

// ───────────────────────── só SQLite ─────────────────────────

#[tokio::test]
async fn team_deletion_cascades_to_everything_it_owns() {
    // I5: apagar a equipe leva canais, mensagens, entregas, tarefas e sessões junto.
    let store = Store::open_in_memory().await.unwrap();
    let t = team("A", 1);
    store.create_team(&t).await.unwrap();
    let a = add_agent(&store, &t, "backend").await;
    let (tid, aid) = (t.id.as_str(), a.id.as_str());
    for (sql, binds) in [
        ("INSERT INTO channels (id, team_id, slug, created_at) VALUES ('ch1', ?, 'geral', 0)", vec![tid]),
        ("INSERT INTO messages (id, team_id, kind, from_kind, from_agent, broadcast, body, created_at) VALUES ('m1', ?, 'message', 'agent', ?, 1, 'oi', 0)", vec![tid, aid]),
        ("INSERT INTO deliveries (message_id, agent_id) VALUES ('m1', ?)", vec![aid]),
        ("INSERT INTO tasks (id, team_id, title, created_at, updated_at) VALUES ('t1', ?, 'x', 0, 0)", vec![tid]),
        ("INSERT INTO sessions (id, agent_id, started_at, log_path) VALUES ('s1', ?, 0, '/tmp/x.log')", vec![aid]),
    ] {
        let mut q = sqlx::query(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.execute(store.pool()).await.unwrap();
    }

    store.delete_team(&t.id).await.unwrap();
    for table in [
        "agents",
        "channels",
        "messages",
        "deliveries",
        "tasks",
        "sessions",
    ] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(n, 0, "{table} should be empty after deleting the team");
    }
}

#[tokio::test]
async fn invalid_data_in_the_database_is_reported_not_trusted() {
    let store = Store::open_in_memory().await.unwrap();
    let t = team("A", 1);
    store.create_team(&t).await.unwrap();
    let a = add_agent(&store, &t, "backend").await;

    sqlx::query("UPDATE agents SET delivery_mode = 'shout' WHERE id = ?")
        .bind(a.id.as_str())
        .execute(store.pool())
        .await
        .unwrap();
    let err = store.get_agent(&a.id).await.unwrap_err();
    assert!(
        matches!(err, RepoError::Corrupt(ref m) if m.contains("delivery_mode")),
        "{err:?}"
    );

    // Um handle editado à mão para um valor reservado também não entra no domínio.
    sqlx::query("UPDATE agents SET delivery_mode = 'pull', handle = 'all' WHERE id = ?")
        .bind(a.id.as_str())
        .execute(store.pool())
        .await
        .unwrap();
    assert!(matches!(
        store.get_agent(&a.id).await,
        Err(RepoError::Corrupt(_))
    ));
}

#[tokio::test]
async fn data_survives_closing_and_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aisense.db");
    let t = team("Squad Produto", 1);
    {
        let store = Store::open(&path).await.unwrap();
        store.create_team(&t).await.unwrap();
        for h in ["backend", "frontend", "revisor"] {
            add_agent(&store, &t, h).await;
        }
        store.close().await;
    }
    let store = Store::open(&path).await.unwrap();
    assert_eq!(store.get_team(&t.id).await.unwrap(), Some(t.clone()));
    assert_eq!(store.list_agents(&t.id).await.unwrap().len(), 3);
}

#[tokio::test]
async fn board_with_1000_cards_loads_in_under_20ms() {
    // Aceite da F06-02: o quadro inteiro (cartões, dependências, comentários) de uma vez.
    let store = Store::open_in_memory().await.unwrap();
    let t = team("Grande", 1);
    store.create_team(&t).await.unwrap();
    let a = add_agent(&store, &t, "backend").await;
    let board = ensure_board(&store, &t.id, 1).await.unwrap();
    let columns = store.list_columns(&board.id).await.unwrap();
    let mut ids = Vec::new();
    for i in 0..1_000i64 {
        let column = &columns[usize::try_from(i).unwrap() % columns.len()].id;
        let mut c = card(&t, column, &format!("Cartão {i}"), i);
        c.labels = vec!["backend".into(), format!("l{}", i % 7)];
        c.checklist = vec![ChecklistItem {
            text: "item".into(),
            done: i % 2 == 0,
        }];
        if i % 3 == 0 {
            c.assignee = Some(a.id.clone());
        }
        store.insert_card(&c).await.unwrap();
        ids.push(c.id);
    }
    for pair in ids.windows(2).step_by(10) {
        store.add_dependency(&pair[1], &pair[0]).await.unwrap();
    }
    // Primeira leitura prepara as instruções (como na linha do tempo, F05-02); mede a segunda.
    store
        .list_cards(&t.id, &CardQuery::default())
        .await
        .unwrap();
    let started = std::time::Instant::now();
    let cards = store
        .list_cards(&t.id, &CardQuery::default())
        .await
        .unwrap();
    let deps = store.dependencies(&t.id).await.unwrap();
    let counts = store.comment_counts(&t.id).await.unwrap();
    let elapsed = started.elapsed();
    assert_eq!(cards.len(), 1_000);
    assert_eq!(deps.len(), 100);
    assert!(counts.is_empty());
    // O aceite (<20 ms) vale para o binário otimizado (~7 ms medidos); sem otimização o
    // parse do JSON sozinho leva ~12 ms, então o teto aqui é folgado só no build de debug.
    let limit = if cfg!(debug_assertions) { 80 } else { 20 };
    assert!(elapsed.as_millis() < limit, "quadro levou {elapsed:?}");
}

#[tokio::test]
async fn board_migration_runs_on_an_existing_database() {
    // Banco de um usuário na versão 4, com uma equipe e uma tarefa antiga (só `status`).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aisense.db");
    let t = team("Antiga", 1);
    {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .unwrap();
        let until_4 = sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Owned(
                aisense_store::MIGRATOR
                    .iter()
                    .filter(|m| m.version <= 4)
                    .cloned()
                    .collect(),
            ),
            ..sqlx::migrate::Migrator::DEFAULT
        };
        until_4.run(&pool).await.unwrap();
        sqlx::query("INSERT INTO teams (id, name, workdir, created_at, updated_at) VALUES (?, 'Antiga', '/tmp', 1, 1)")
            .bind(t.id.as_str())
            .execute(&pool)
            .await
            .unwrap();
        for (id, status) in [
            ("tsk_old1", "doing"),
            ("tsk_old2", "cancelled"),
            ("tsk_old3", "estranho"),
        ] {
            sqlx::query("INSERT INTO tasks (id, team_id, title, status, created_at, updated_at) VALUES (?, ?, 'antiga', ?, 7, 7)")
                .bind(id)
                .bind(t.id.as_str())
                .bind(status)
                .execute(&pool)
                .await
                .unwrap();
        }
        pool.close().await;
    }
    let store = Store::open(&path).await.unwrap();
    let board = ensure_board(&store, &t.id, 9).await.unwrap();
    let columns = store.list_columns(&board.id).await.unwrap();
    let slug_of = |c: &Card| {
        columns
            .iter()
            .find(|col| col.id == c.column_id)
            .unwrap()
            .slug
            .clone()
    };
    let cards = store
        .list_cards(&t.id, &CardQuery::default())
        .await
        .unwrap();
    let placed: Vec<(String, String)> = cards
        .iter()
        .map(|c| (c.id.to_string(), slug_of(c)))
        .collect();
    assert_eq!(
        placed,
        [
            ("tsk_old3".to_owned(), "todo".to_owned()),
            ("tsk_old1".to_owned(), "doing".to_owned()),
            ("tsk_old2".to_owned(), "done".to_owned()),
        ]
    );
    assert_eq!(cards[0].version, 1);
    assert_eq!(cards[0].column_since, 7);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn claim_is_atomic_across_connections() {
    // F06-03 no SQLite de verdade: arquivo com WAL e várias conexões no pool.
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("aisense.db")).await.unwrap();
    let t = team("Squad", 1);
    store.create_team(&t).await.unwrap();
    let mut agents = Vec::new();
    for i in 0..8 {
        agents.push(add_agent(&store, &t, &format!("agente{i}")).await.id);
    }
    let board = ensure_board(&store, &t.id, 1).await.unwrap();
    let todo = store.list_columns(&board.id).await.unwrap()[1].id.clone();
    let c = card(&t, &todo, "disputado", 0);
    store.insert_card(&c).await.unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    let mut tasks = Vec::new();
    for agent in agents {
        let (store, c, barrier) = (store.clone(), c.clone(), barrier.clone());
        tasks.push(tokio::spawn(async move {
            let mut mine = c.clone();
            mine.assignee = Some(agent);
            mine.version = 2;
            barrier.wait().await;
            store
                .update_card(
                    &mine,
                    &WriteGuard {
                        require_unassigned: true,
                        ..guard(1)
                    },
                )
                .await
                .unwrap()
        }));
    }
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(
        results.iter().filter(|r| **r == CardWrite::Written).count(),
        1,
        "{results:?}"
    );
    assert_eq!(
        results.iter().filter(|r| **r == CardWrite::Stale).count(),
        7
    );
}
