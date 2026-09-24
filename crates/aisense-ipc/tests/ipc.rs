//! Servidor e cliente de verdade, num socket (ou pipe) temporário.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use aisense_core::agent::{create_agent, AgentDraft, AgentState};
use aisense_core::bus::{BusService, NoObserver};
use aisense_core::repo::{
    InMemoryStore, SessionRecord, SessionRepository, TeamRepository, TokenRecord, TokenRepository,
};
use aisense_core::team::{Team, TeamDraft};
use aisense_core::{AgentId, SessionId};
use aisense_ipc::{serve, BusHandler, Client, ClientError, Request, Response, MAX_FRAME};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

struct World {
    endpoint: String,
    store: Arc<InMemoryStore>,
    tokens: Vec<(String, SessionId)>,
    ids: Vec<AgentId>,
    shutdown: CancellationToken,
    _dir: tempfile::TempDir,
}

impl Drop for World {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

fn endpoint(dir: &tempfile::TempDir) -> String {
    if cfg!(windows) {
        format!(r"\\.\pipe\aisense-test-{}", ulid::Ulid::new())
    } else {
        dir.path()
            .join("run")
            .join("aisense.sock")
            .display()
            .to_string()
    }
}

async fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(InMemoryStore::new());
    let team = Team::create(
        &TeamDraft {
            name: "Squad".into(),
            workdir: dir.path().display().to_string(),
            ..TeamDraft::default()
        },
        1,
    )
    .unwrap();
    store.create_team(&team).await.unwrap();
    let mut tokens = Vec::new();
    let mut ids = Vec::new();
    for handle in ["backend", "frontend"] {
        let draft = AgentDraft {
            handle: handle.into(),
            name: handle.into(),
            adapter_id: "shell".into(),
            ..AgentDraft::default()
        };
        let agent = create_agent(&*store, &team.id, &draft, 1).await.unwrap();
        let session = SessionRecord {
            id: SessionId::new(),
            agent_id: agent.id.clone(),
            pid: None,
            started_at: 1,
            ended_at: None,
            exit_code: None,
            log_path: "/tmp/x.log".into(),
            log_offset: None,
        };
        store.start_session(&session).await.unwrap();
        let token = format!("token-{handle}");
        store
            .insert_token(
                &token,
                &TokenRecord {
                    agent_id: agent.id.clone(),
                    session_id: session.id.clone(),
                    expires_at: i64::MAX,
                },
            )
            .await
            .unwrap();
        tokens.push((token, session.id));
        ids.push(agent.id);
    }
    let bus = BusService::new(
        Arc::clone(&store),
        Arc::new(|_: &AgentId| AgentState::Idle),
        Arc::new(NoObserver),
    );
    let endpoint = endpoint(&dir);
    let shutdown = CancellationToken::new();
    let handler = Arc::new(BusHandler::new(bus));
    let (ep, stop) = (endpoint.clone(), shutdown.clone());
    tokio::spawn(async move { serve(&ep, handler, stop).await.unwrap() });
    // Espera o socket existir.
    for _ in 0..100 {
        if aisense_ipc::connect(&endpoint).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    World {
        endpoint,
        store,
        tokens,
        ids,
        shutdown,
        _dir: dir,
    }
}

impl World {
    async fn client(&self, i: usize) -> Client {
        Client::connect(&self.endpoint, &self.tokens[i].0)
            .await
            .unwrap()
    }
}

fn data(response: Response) -> serde_json::Value {
    assert!(response.ok, "{response:?}");
    response.data.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn dois_agentes_trocam_recado_pelo_socket() {
    let w = world().await;
    let mut backend = w.client(0).await;
    let hello = backend.hello.data.clone().unwrap();
    assert_eq!(hello["identity"]["handle"], "backend");
    assert_eq!(hello["agents"].as_array().unwrap().len(), 2);

    let sent = data(
        backend
            .call(&Request::Send {
                to: vec!["@frontend".into()],
                body: "contrato subiu".into(),
                subject: None,
                meta: Default::default(),
            })
            .await
            .unwrap(),
    );
    assert_eq!(sent["ids"].as_array().unwrap().len(), 1);

    let mut frontend = w.client(1).await;
    let inbox = data(
        frontend
            .call(&Request::Inbox { drain: true })
            .await
            .unwrap(),
    );
    assert_eq!(inbox[0]["from"], "@backend");
    assert_eq!(inbox[0]["to"], "@frontend");
    assert_eq!(inbox[0]["body"], "contrato subiu");
    let again = data(
        frontend
            .call(&Request::Inbox { drain: false })
            .await
            .unwrap(),
    );
    assert_eq!(again.as_array().unwrap().len(), 0, "drain marcou como lida");

    // Erro do core chega com código e dica, e a conexão continua.
    let unknown = frontend
        .call(&Request::Send {
            to: vec!["@designer".into()],
            body: "oi".into(),
            subject: None,
            meta: Default::default(),
        })
        .await
        .unwrap();
    assert_eq!(unknown.error.as_deref(), Some("unknown_agent"));
    assert!(unknown.hint.unwrap().contains("aisense agents"));
    let agents = data(frontend.call(&Request::Agents).await.unwrap());
    assert_eq!(agents[0]["handle"], "backend");
}

#[tokio::test(flavor = "multi_thread")]
async fn wait_acorda_quando_a_mensagem_chega() {
    let w = world().await;
    let mut frontend = w.client(1).await;
    let waiting = tokio::spawn(async move {
        frontend
            .call(&Request::Wait {
                timeout_s: Some(10),
            })
            .await
            .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut backend = w.client(0).await;
    backend
        .call(&Request::Send {
            to: vec!["@all".into()],
            body: "reunião".into(),
            subject: None,
            meta: Default::default(),
        })
        .await
        .unwrap();
    let got = data(
        tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .unwrap()
            .unwrap(),
    );
    assert_eq!(got["body"], "reunião");
    assert_eq!(got["to"], "@all");

    let mut again = w.client(1).await;
    let timeout = again
        .call(&Request::Wait { timeout_s: Some(0) })
        .await
        .unwrap();
    assert_eq!(timeout.error.as_deref(), Some("timeout"));
}

#[tokio::test(flavor = "multi_thread")]
async fn sem_token_valido_a_conexao_e_recusada_e_fechada() {
    let w = world().await;
    match Client::connect(&w.endpoint, "falso").await {
        Err(ClientError::Refused(r)) => assert_eq!(r.error.as_deref(), Some("unauthorized")),
        other => panic!("esperava recusa: {:?}", other.err()),
    }

    // Primeiro frame que não é hello: recusa e fecha.
    let mut io = aisense_ipc::connect(&w.endpoint).await.unwrap();
    io.write_all(b"{\"op\":\"agents\"}\n").await.unwrap();
    let mut answer = String::new();
    io.read_to_string(&mut answer).await.unwrap();
    assert!(answer.contains("\"unauthorized\""), "{answer}");
}

#[tokio::test(flavor = "multi_thread")]
async fn token_de_sessao_encerrada_e_recusado_na_hora() {
    let w = world().await;
    let mut backend = w.client(0).await;
    assert!(backend.call(&Request::Agents).await.unwrap().ok);
    // Fim da sessão: o supervisor revoga (F05-04). A conexão aberta perde o acesso já.
    w.store.revoke_session_tokens(&w.tokens[0].1).await.unwrap();
    let after = backend.call(&Request::Agents).await.unwrap();
    assert_eq!(after.error.as_deref(), Some("unauthorized"));
    assert!(Client::connect(&w.endpoint, &w.tokens[0].0).await.is_err());
    let _ = &w.ids;
}

#[tokio::test(flavor = "multi_thread")]
async fn frame_grande_demais_e_recusado_sem_derrubar_o_servidor() {
    let w = world().await;
    let mut io = aisense_ipc::connect(&w.endpoint).await.unwrap();
    let hello = format!("{{\"op\":\"hello\",\"token\":\"{}\"}}\n", w.tokens[0].0);
    io.write_all(hello.as_bytes()).await.unwrap();
    let big = vec![b'x'; MAX_FRAME + 10];
    // O servidor pode fechar antes de ler tudo: erro de escrita aqui é esperado.
    let _ = io.write_all(&big).await;
    let _ = io.write_all(b"\n").await;
    let mut answer = String::new();
    let _ = io.read_to_string(&mut answer).await;
    assert!(answer.contains("frame_too_large"), "{answer}");

    // E continua atendendo.
    let mut other = w.client(1).await;
    assert!(other.call(&Request::Agents).await.unwrap().ok);
    // Lixo que não é JSON: erro, mas a conexão segue.
    let mut raw = aisense_ipc::connect(&w.endpoint).await.unwrap();
    let hello = format!(
        "{{\"op\":\"hello\",\"token\":\"{}\"}}\nnão é json\n{{\"op\":\"whoami\"}}\n",
        w.tokens[1].0
    );
    raw.write_all(hello.as_bytes()).await.unwrap();
    raw.shutdown().await.unwrap();
    let mut answer = String::new();
    raw.read_to_string(&mut answer).await.unwrap();
    let lines: Vec<_> = answer.lines().collect();
    assert_eq!(lines.len(), 3, "{answer}");
    assert!(lines[1].contains("invalid_request"));
    assert!(lines[2].contains("\"frontend\""));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn socket_so_do_usuario() {
    use std::os::unix::fs::PermissionsExt;
    let w = world().await;
    let mode = std::fs::metadata(&w.endpoint).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let dir = std::path::Path::new(&w.endpoint).parent().unwrap();
    assert_eq!(
        std::fs::metadata(dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    // Um segundo servidor no mesmo socket é recusado.
    let bus = BusService::new(
        Arc::clone(&w.store),
        Arc::new(|_: &AgentId| AgentState::Idle),
        Arc::new(NoObserver),
    );
    let second = serve(
        &w.endpoint,
        Arc::new(BusHandler::new(bus)),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(second, Err(aisense_ipc::BindError::InUse(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn notas_pelo_socket_append_simultaneo_nao_perde_nada() {
    use aisense_ipc::NotesOp;
    let w = world().await;
    let mut setup = w.client(0).await;
    let created = setup
        .call(&Request::Notes(NotesOp::New {
            slug: "log".into(),
            title: "Log".into(),
        }))
        .await
        .unwrap();
    assert!(created.ok, "{created:?}");

    // CLI e MCP mandam o mesmo frame; aqui, 8 conexões dos dois agentes ao mesmo tempo.
    let mut tasks = Vec::new();
    for c in 0..8 {
        let client = w.client(c % 2).await;
        tasks.push(tokio::spawn(async move {
            let mut client = client;
            for i in 0..25 {
                let r = client
                    .call(&Request::Notes(NotesOp::Append {
                        slug: "log".into(),
                        text: format!("c{c} linha {i}"),
                    }))
                    .await
                    .unwrap();
                assert!(r.ok, "{r:?}");
            }
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
    let read = setup
        .call(&Request::Notes(NotesOp::Read {
            slug: "log".into(),
            section: None,
        }))
        .await
        .unwrap();
    let content = data(read)["content"].as_str().unwrap().to_owned();
    assert_eq!(content.lines().count(), 1 + 8 * 25);

    // `write` com hash velho: stale_note com o diff, pelo socket também.
    let stale = setup
        .call(&Request::Notes(NotesOp::Write {
            slug: "log".into(),
            content: "# Log\n".into(),
            expect_hash: Some("0000".into()),
        }))
        .await
        .unwrap();
    assert_eq!(stale.error.as_deref(), Some("stale_note"));
    assert!(stale.data.unwrap()["diff"].is_array());
    let search = setup
        .call(&Request::Notes(NotesOp::Search {
            query: "c3 linha 24".into(),
        }))
        .await
        .unwrap();
    assert_eq!(data(search).as_array().unwrap().len(), 1);
}
