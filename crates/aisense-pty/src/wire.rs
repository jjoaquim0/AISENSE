//! Tipos que cruzam a fronteira para o TypeScript.
//!
//! Moram aqui, e não no crate do Tauri, porque `ts-rs` exporta durante os testes:
//! se estivessem lá, gerar tipos exigiria compilar uma janela — o que no Linux
//! precisa de WebKit/GTK instalados. Ver regra R5 em `AGENTS.md`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Saída de um terminal chegando na interface.
///
/// Os bytes vão em **base64**, e não como texto: um caractere UTF-8 pode ser partido
/// entre duas leituras do PTY, e converter para `String` aqui comeria o caractere
/// (acentos e emoji virariam `�`). O front decodifica para `Uint8Array` e entrega ao
/// xterm, que mantém um decodificador incremental.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PtyData {
    pub agent_id: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PtyExit {
    pub agent_id: String,
    pub code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SpawnRequest {
    pub agent_id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: Vec<(String, String)>,
    #[serde(default)]
    pub log_path: Option<String>,
    pub rows: u16,
    pub cols: u16,
}

impl From<SpawnRequest> for crate::session::PtySpawn {
    fn from(request: SpawnRequest) -> Self {
        Self {
            command: request.command,
            args: request.args,
            cwd: request.cwd.map(Into::into),
            env: request.env,
            size: crate::session::TerminalSize {
                rows: request.rows,
                cols: request.cols,
            },
            log_path: request.log_path.map(Into::into),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn converte_a_requisicao_em_especificacao_de_spawn() {
        let request = SpawnRequest {
            agent_id: "agt_1".into(),
            command: "sh".into(),
            args: vec!["-c".into(), "echo oi".into()],
            cwd: Some("/tmp".into()),
            env: vec![("AISENSE_AGENT_HANDLE".into(), "backend".into())],
            log_path: None,
            rows: 40,
            cols: 132,
        };

        let spec: crate::session::PtySpawn = request.into();
        assert_eq!(spec.command, "sh");
        assert_eq!(spec.size.rows, 40);
        assert_eq!(spec.size.cols, 132);
        assert_eq!(spec.env.len(), 1);
    }

    #[test]
    fn os_campos_opcionais_tem_padrao_no_json() {
        // O front pode omitir args/env/cwd; omitir não pode virar erro de parse.
        let request: SpawnRequest =
            serde_json::from_str(r#"{"agentId":"agt_1","command":"bash","rows":24,"cols":80}"#)
                .unwrap();
        assert!(request.args.is_empty());
        assert!(request.cwd.is_none());
    }
}
