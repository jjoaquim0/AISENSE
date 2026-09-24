//! Injeção do `BOOT.md` na subida do agente (F04-06, `docs/06` "Ciclo de vida", passo 4).
//!
//! Três caminhos, do melhor para o pior:
//! 1. **Flag de system prompt** (`capabilities.system_prompt_flag`): o texto vai como
//!    argumento. Não suja o histórico e chega antes da primeira tela.
//! 2. **MCP**: o `aisense-mcp` entrega o `BOOT.md` como `instructions` do `initialize`, que
//!    o cliente põe no system prompt. Só vale quando o servidor existir (F05-09); até lá o
//!    supervisor não o escolhe (`SupervisorConfig::mcp_boot`).
//! 3. **Terminal** (`inject.mode = "stdin"`): espera o primeiro `idle` com confiança alta e
//!    digita "leia o BOOT.md e siga". Nunca digita com o agente aguardando o humano; depois
//!    de 30 s sem prompt, desiste e avisa.
//!
//! Qual foi usado — e se deu certo — fica registrado no supervisor e vai para a UI.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::adapter::{Adapter, InjectMode};

/// Quanto o caminho pelo terminal espera o primeiro prompt (`docs/fases/FASE-04`, riscos).
pub const STDIN_BOOT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum BootChannel {
    SystemPromptFlag {
        flag: String,
    },
    Mcp,
    Stdin,
    /// O runtime não recebe o `BOOT.md` (shell puro, comando customizado).
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum BootStatus {
    /// Entregue (ou vai junto com o processo, no caso da flag e do MCP).
    Delivered,
    /// Pelo terminal, esperando o primeiro prompt.
    Waiting,
    /// Nada a entregar por este runtime.
    Skipped,
    /// Não chegou: o agente sobe, mas sem a identidade. A mensagem diz o que fazer.
    Failed,
}

/// O que aconteceu com o `BOOT.md` na sessão atual de um agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct BootDelivery {
    pub channel: BootChannel,
    pub status: BootStatus,
    /// Frase pronta em pt-BR.
    pub message: String,
}

impl BootDelivery {
    pub fn new(channel: BootChannel, status: BootStatus, message: impl Into<String>) -> Self {
        Self {
            channel,
            status,
            message: message.into(),
        }
    }
}

/// O melhor caminho que o adaptador suporta. `mcp_boot` diz se o servidor MCP já entrega
/// o `BOOT.md` (F05-09).
pub fn choose_boot_channel(adapter: &Adapter, mcp_boot: bool) -> BootChannel {
    if let Some(flag) = &adapter.capabilities.system_prompt_flag {
        return BootChannel::SystemPromptFlag { flag: flag.clone() };
    }
    if adapter.capabilities.mcp && adapter.capabilities.mcp_config.is_some() && mcp_boot {
        return BootChannel::Mcp;
    }
    if adapter.inject.mode == InjectMode::Stdin && adapter.inject.boot {
        return BootChannel::Stdin;
    }
    BootChannel::None
}

/// Caminho do `BOOT.md` relativo ao diretório de trabalho — que é o `cwd` do agente.
pub fn boot_relative_path(handle: &str) -> String {
    format!(
        "{}/agents/{handle}/{}",
        crate::skill::AISENSE_DIR,
        crate::skill::BOOT_FILE
    )
}

/// O que o caminho pelo terminal digita: curto, para caber em qualquer prompt, e com o
/// `submit` do adaptador no fim.
pub fn stdin_boot_text(adapter: &Adapter, handle: &str) -> String {
    format!(
        "{}Leia {} e siga as instruções: é quem você é nesta equipe.{}",
        adapter.inject.prefix,
        boot_relative_path(handle),
        adapter.inject.submit
    )
}

/// Frase de quando o caminho pelo terminal desiste.
pub fn stdin_timeout_message(handle: &str) -> String {
    format!(
        "o terminal de @{handle} não mostrou o prompt em {} s; o BOOT.md não foi enviado. \
         Peça ao agente para ler {}",
        STDIN_BOOT_TIMEOUT.as_secs(),
        boot_relative_path(handle)
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::adapter::{AdapterCatalog, BUILTIN_ADAPTERS};

    fn adapter(id: &str) -> Adapter {
        AdapterCatalog::load_from(BUILTIN_ADAPTERS, None)
            .get(id)
            .cloned()
            .unwrap()
    }

    #[test]
    fn cada_runtime_embutido_usa_o_melhor_caminho_que_tem() {
        let flag = |f: &str| BootChannel::SystemPromptFlag { flag: f.into() };
        assert_eq!(
            choose_boot_channel(&adapter("claude"), false),
            flag("--append-system-prompt")
        );
        assert_eq!(
            choose_boot_channel(&adapter("opencode"), false),
            flag("--prompt")
        );
        // Sem flag: MCP só onde o AISENSE registra o servidor (`mcp_config`); o codex tem
        // MCP, mas a configuração dele é global do usuário — segue pelo terminal.
        assert_eq!(
            choose_boot_channel(&adapter("codex"), false),
            BootChannel::Stdin
        );
        assert_eq!(
            choose_boot_channel(&adapter("codex"), true),
            BootChannel::Stdin
        );
        let mut with_mcp = adapter("codex");
        with_mcp.capabilities.mcp_config = Some(".mcp.json".into());
        assert_eq!(choose_boot_channel(&with_mcp, true), BootChannel::Mcp);
        assert_eq!(choose_boot_channel(&with_mcp, false), BootChannel::Stdin);
        // A flag vence o MCP.
        assert_eq!(
            choose_boot_channel(&adapter("claude"), true),
            flag("--append-system-prompt")
        );
        // Shell e comando customizado não têm quem leia.
        assert_eq!(
            choose_boot_channel(&adapter("shell"), true),
            BootChannel::None
        );
        assert_eq!(
            choose_boot_channel(&adapter("custom"), true),
            BootChannel::None
        );
    }

    #[test]
    fn o_texto_do_terminal_aponta_o_boot_do_agente_e_submete() {
        let text = stdin_boot_text(&adapter("codex"), "revisor");
        assert!(text.starts_with("[AISENSE] Leia .aisense/agents/revisor/BOOT.md"));
        assert!(text.ends_with('\r'));
        assert!(
            !text[..text.len() - 1].contains(['\r', '\n']),
            "uma linha só"
        );
    }
}
