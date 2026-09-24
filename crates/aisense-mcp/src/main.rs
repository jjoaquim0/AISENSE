//! Binário `aisense-mcp`: servidor MCP stdio que expõe o barramento como ferramentas
//! nativas para runtimes compatíveis (`docs/07`, F05-09).
//!
//! É um cliente do mesmo socket que a CLI usa: nenhuma regra mora aqui. JSON-RPC 2.0 por
//! linha em stdin/stdout, só o necessário do MCP: `initialize`, `tools/list`, `tools/call`
//! e `ping`. O `initialize` devolve o `BOOT.md` do agente como `instructions` — é por aí
//! que a identidade chega a runtimes sem flag de system prompt (F04-06).
#![forbid(unsafe_code)]

mod tools;

use aisense_ipc::Client;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Versões do protocolo MCP que este servidor fala; a primeira é a preferida.
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

fn main() {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("aisense-mcp: {e}");
            std::process::exit(1);
        }
    };
    runtime.block_on(serve());
}

async fn serve() {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    let mut client: Option<Client> = None;
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            write(&mut stdout, &rpc_error(Value::Null, -32700, "parse error")).await;
            continue;
        };
        // Notificações (sem id) não têm resposta.
        let Some(id) = message.get("id").cloned() else {
            continue;
        };
        let method = message["method"].as_str().unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let reply = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": initialize(&params),
            }),
            "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tools::list() },
            }),
            "tools/call" => {
                let result = call(&mut client, &params).await;
                json!({ "jsonrpc": "2.0", "id": id, "result": result })
            }
            _ => rpc_error(id, -32601, "method not found"),
        };
        write(&mut stdout, &reply).await;
    }
}

fn initialize(params: &Value) -> Value {
    let asked = params["protocolVersion"].as_str().unwrap_or_default();
    let version = PROTOCOL_VERSIONS
        .iter()
        .find(|v| **v == asked)
        .unwrap_or(&PROTOCOL_VERSIONS[0]);
    let mut result = json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "aisense", "version": aisense_core::VERSION },
    });
    if let Some(boot) = std::env::var("AISENSE_BOOT_FILE")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
    {
        result["instructions"] = Value::String(boot);
    }
    result
}

/// Uma ferramenta: monta o frame, manda pelo socket e devolve o texto que a CLI mostraria.
async fn call(client: &mut Option<Client>, params: &Value) -> Value {
    let name = params["name"].as_str().unwrap_or_default();
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let request = match tools::request(name, &args) {
        Ok(request) => request,
        Err(message) => return tool_error(&message),
    };
    let op = request.op();
    if client.is_none() {
        match connect().await {
            Ok(connected) => *client = Some(connected),
            Err(message) => return tool_error(&message),
        }
    }
    let Some(connection) = client.as_mut() else {
        return tool_error("sem conexão com o AISENSE");
    };
    let response = match connection.call(&request).await {
        Ok(response) => response,
        Err(e) => {
            // Conexão caída (app reiniciado): tenta de novo na próxima chamada.
            *client = None;
            return tool_error(&format!("falha no barramento: {e}"));
        }
    };
    if !response.ok {
        let mut message = response
            .message
            .unwrap_or_else(|| "falha no barramento".into());
        if let Some(hint) = response.hint {
            message.push_str(&format!("\nDica: {hint}"));
        }
        return tool_error(&message);
    }
    let data = response.data.unwrap_or(Value::Null);
    let text = aisense_ipc::render::render(op, &data, false);
    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
}

async fn connect() -> Result<Client, String> {
    let (Ok(socket), Ok(token)) = (
        std::env::var("AISENSE_SOCKET"),
        std::env::var("AISENSE_TOKEN"),
    ) else {
        return Err(
            "este runtime não foi aberto pelo AISENSE (AISENSE_SOCKET/AISENSE_TOKEN ausentes)"
                .into(),
        );
    };
    Client::connect(&socket, &token)
        .await
        .map_err(|e| format!("não consegui falar com o AISENSE: {e}"))
}

fn tool_error(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

async fn write(stdout: &mut tokio::io::Stdout, value: &Value) {
    let mut line = value.to_string();
    line.push('\n');
    let _ = stdout.write_all(line.as_bytes()).await;
    let _ = stdout.flush().await;
}
