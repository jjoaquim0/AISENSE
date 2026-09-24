//! O binário `aisense-mcp` de verdade, falando JSON-RPC por stdio com um barramento real.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use aisense_core::agent::{create_agent, AgentDraft, AgentState};
use aisense_core::bus::{BusRepository, BusService, InboxQuery, NoObserver};
use aisense_core::repo::{
    InMemoryStore, SessionRecord, SessionRepository, TeamRepository, TokenRecord, TokenRepository,
};
use aisense_core::team::{Team, TeamDraft};
use aisense_core::{AgentId, SessionId};
use aisense_ipc::{serve, BusHandler};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout};
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread")]
async fn ferramentas_mcp_conversam_com_o_barramento() {
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
    let mut ids = Vec::new();
    for handle in ["backend", "frontend"] {
        let draft = AgentDraft {
            handle: handle.into(),
            name: handle.into(),
            adapter_id: "codex".into(),
            ..AgentDraft::default()
        };
        ids.push(create_agent(&*store, &team.id, &draft, 1).await.unwrap().id);
    }
    let session = SessionRecord {
        id: SessionId::new(),
        agent_id: ids[0].clone(),
        pid: None,
        started_at: 1,
        ended_at: None,
        exit_code: None,
        log_path: "/tmp/x.log".into(),
        log_offset: None,
    };
    store.start_session(&session).await.unwrap();
    store
        .insert_token(
            "tok",
            &TokenRecord {
                agent_id: ids[0].clone(),
                session_id: session.id,
                expires_at: i64::MAX,
            },
        )
        .await
        .unwrap();

    let endpoint = if cfg!(windows) {
        format!(r"\\.\pipe\aisense-mcp-test-{}", std::process::id())
    } else {
        dir.path().join("run").join("s.sock").display().to_string()
    };
    let bus = BusService::new(
        Arc::clone(&store),
        Arc::new(|_: &AgentId| AgentState::Idle),
        Arc::new(NoObserver),
    );
    let shutdown = CancellationToken::new();
    let (ep, stop) = (endpoint.clone(), shutdown.clone());
    tokio::spawn(async move {
        serve(&ep, Arc::new(BusHandler::from_bus(bus)), stop)
            .await
            .unwrap()
    });
    for _ in 0..100 {
        if aisense_ipc::connect(&endpoint).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let boot = dir.path().join("BOOT.md");
    std::fs::write(&boot, "# Você é @backend — Squad\n").unwrap();

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_aisense-mcp"))
        .env("AISENSE_SOCKET", &endpoint)
        .env("AISENSE_TOKEN", "tok")
        .env("AISENSE_BOOT_FILE", &boot)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let init = call(&mut stdin, &mut stdout, json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}))
        .await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(
        init["result"]["instructions"], "# Você é @backend — Squad\n",
        "o BOOT.md chega como instructions"
    );
    let tools = call(
        &mut stdin,
        &mut stdout,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    assert!(tools["result"]["tools"].as_array().unwrap().len() >= 5);

    let sent = call(&mut stdin, &mut stdout, json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "aisense_send_message", "arguments": {"to": ["@frontend"], "body": "pelo MCP"}}}))
        .await;
    assert_eq!(sent["result"]["isError"], false, "{sent}");
    assert!(sent["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("enviado"));
    let inbox = store
        .inbox(
            &ids[1],
            &InboxQuery {
                unread_only: true,
                after: None,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(inbox[0].message.body, "pelo MCP");

    let err = call(&mut stdin, &mut stdout, json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "aisense_send_message", "arguments": {"to": ["@ninguem"], "body": "x"}}}))
        .await;
    assert_eq!(err["result"]["isError"], true);
    assert!(err["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Dica:"));

    let unknown = call(
        &mut stdin,
        &mut stdout,
        json!({"jsonrpc": "2.0", "id": 5, "method": "resources/list"}),
    )
    .await;
    assert_eq!(unknown["error"]["code"], -32601);
    shutdown.cancel();
}

async fn call(
    stdin: &mut ChildStdin,
    stdout: &mut Lines<BufReader<ChildStdout>>,
    request: Value,
) -> Value {
    stdin
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    let answer = tokio::time::timeout(Duration::from_secs(10), stdout.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(&answer).unwrap()
}
