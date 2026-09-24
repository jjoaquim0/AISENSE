//! Entrega em modo `hook` (F05-08, `docs/07`): o runtime checa a caixa sozinho ao fim de
//! cada turno. Para o Claude Code, um hook `Stop` em `.claude/settings.json` que roda a CLI;
//! com mensagem nova, a CLI responde `{"decision":"block","reason":...}` e o agente continua
//! com as mensagens em mãos, sem injeção e sem depender do detector de estado.
//!
//! O arquivo é do usuário: o hook é **mesclado** (nunca sobrescreve nada), só uma vez, e um
//! JSON que não dá para ler fica intacto (vira ressalva).

use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

/// O que o hook roda.
pub const INBOX_HOOK_COMMAND: &str = "aisense inbox --drain --if-any --hook-json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookInstall {
    Installed,
    AlreadyThere,
}

/// Garante o hook `Stop` em `settings`. Cria o arquivo (e a pasta) se preciso.
pub fn install_inbox_hook(settings: &Path) -> Result<HookInstall, String> {
    let mut root = match fs::read_to_string(settings) {
        Ok(text) if text.trim().is_empty() => Value::Object(Map::new()),
        Ok(text) => serde_json::from_str::<Value>(&text).map_err(|e| {
            format!(
                "{} não é um JSON válido ({e}); o hook de caixa de entrada não foi instalado",
                settings.display()
            )
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(e) => return Err(format!("não consegui ler {}: {e}", settings.display())),
    };
    let not_object = || {
        format!(
            "{} tem um formato inesperado; o hook de caixa de entrada não foi instalado",
            settings.display()
        )
    };
    let hooks = root
        .as_object_mut()
        .ok_or_else(not_object)?
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(not_object)?;
    let stop = hooks
        .entry("Stop")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(not_object)?;
    let present = stop.iter().any(|group| {
        group["hooks"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|h| h["command"].as_str() == Some(INBOX_HOOK_COMMAND))
    });
    if present {
        return Ok(HookInstall::AlreadyThere);
    }
    stop.push(json!({ "hooks": [{ "type": "command", "command": INBOX_HOOK_COMMAND }] }));

    if let Some(dir) = settings.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("não consegui criar {}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())? + "\n";
    let tmp = settings.with_extension("json.aisense-tmp");
    fs::write(&tmp, text).map_err(|e| format!("não consegui gravar {}: {e}", tmp.display()))?;
    fs::rename(&tmp, settings)
        .map_err(|e| format!("não consegui gravar {}: {e}", settings.display()))?;
    Ok(HookInstall::Installed)
}

/// O que a CLI imprime no modo `--hook-json`: nada sem mensagem (o agente pode parar);
/// com mensagem, o bloqueio com as mensagens como motivo.
pub fn hook_output(messages_text: &str) -> Option<String> {
    let text = messages_text.trim();
    (!text.is_empty()).then(|| {
        json!({
            "decision": "block",
            "reason": format!(
                "Chegaram mensagens da sua equipe no AISENSE. Leia e aja; responda perguntas com aisense reply.\n\n{text}"
            ),
        })
        .to_string()
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn mescla_sem_tocar_no_que_e_do_usuario_e_so_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude").join("settings.json");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        let user = json!({
            "model": "opus",
            "permissions": { "allow": ["Bash(ls)"] },
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "say pronto" }] }],
                "PreToolUse": [{ "matcher": "Bash", "hooks": [] }]
            }
        });
        fs::write(&file, serde_json::to_string(&user).unwrap()).unwrap();

        assert_eq!(install_inbox_hook(&file).unwrap(), HookInstall::Installed);
        assert_eq!(
            install_inbox_hook(&file).unwrap(),
            HookInstall::AlreadyThere
        );
        let now: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(now["model"], "opus");
        assert_eq!(now["permissions"], user["permissions"]);
        assert_eq!(now["hooks"]["PreToolUse"], user["hooks"]["PreToolUse"]);
        let stop = now["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "o do usuário continua lá");
        assert_eq!(stop[0], user["hooks"]["Stop"][0]);
        assert_eq!(stop[1]["hooks"][0]["command"], INBOX_HOOK_COMMAND);
    }

    #[test]
    fn cria_quando_nao_existe_e_nao_mexe_em_json_quebrado() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude").join("settings.json");
        install_inbox_hook(&file).unwrap();
        let created: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(created["hooks"]["Stop"][0]["hooks"][0]["type"], "command");

        fs::write(&file, "{ isto não é json").unwrap();
        let err = install_inbox_hook(&file).unwrap_err();
        assert!(err.contains("não é um JSON válido"));
        assert_eq!(fs::read_to_string(&file).unwrap(), "{ isto não é json");
        fs::write(&file, "[1, 2]").unwrap();
        assert!(install_inbox_hook(&file).is_err());
    }

    #[test]
    fn saida_do_hook_bloqueia_so_com_mensagem() {
        assert_eq!(hook_output("  \n"), None);
        let out: Value = serde_json::from_str(&hook_output("── @x → @y\noi").unwrap()).unwrap();
        assert_eq!(out["decision"], "block");
        assert!(out["reason"].as_str().unwrap().contains("── @x → @y\noi"));
    }
}
