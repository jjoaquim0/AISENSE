//! Preferências do aplicativo (T9 — Configurações; F08-05).
//!
//! Um arquivo só, `settings.json` no diretório de dados, escrito de forma atômica
//! (arquivo temporário + `rename`): uma queda no meio da gravação nunca deixa meio JSON.
//!
//! Todo campo tem padrão (`#[serde(default)]` em cada nível). Um arquivo antigo, sem as
//! seções novas, carrega com os padrões delas; um campo desconhecido é ignorado, para
//! uma versão anterior do app conseguir abrir o arquivo de uma mais nova. Valores fora
//! da faixa são trazidos para dentro dela em [`AppSettings::normalized`] — quem lê as
//! preferências nunca precisa checar de novo.
//!
//! Segredos **não** moram aqui (R3): só o nome da variável de ambiente e o runtime. O
//! valor vai para o keychain do SO, pela porta [`SecretVault`].

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::bus::GuardConfig;
use crate::ids::TeamId;

/// Versão do formato do arquivo. Sobe quando um campo muda de significado.
pub const SETTINGS_VERSION: u32 = 1;

pub const TERMINAL_FONT_SIZE_RANGE: std::ops::RangeInclusive<u16> = 10..=24;
pub const ASK_DEFAULT_SECS_RANGE: std::ops::RangeInclusive<u32> = 10..=1_800;
pub const RETENTION_DAYS_RANGE: std::ops::RangeInclusive<u32> = 1..=3_650;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AppearanceSettings {
    pub theme: ThemePreference,
    pub density: Density,
    pub terminal_font_size: u16,
    /// Vazio = a fonte mono do design system (JetBrains Mono).
    pub terminal_font_family: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            density: Density::Comfortable,
            terminal_font_size: 13,
            terminal_font_family: String::new(),
        }
    }
}

/// Persistência de sessão (F08-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SessionSettings {
    /// Reabrir a última equipe aberta ao subir o app.
    pub restore_last_team: bool,
    /// Religar os agentes que estavam rodando quando o app fechou.
    pub relaunch_agents: bool,
    /// Equipe aberta por último (mantida pelo app, não pelo formulário).
    pub last_team: Option<TeamId>,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            restore_last_team: true,
            relaunch_agents: false,
            last_team: None,
        }
    }
}

/// Notificações do SO (F08-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct NotificationSettings {
    pub enabled: bool,
    /// Agente parou esperando o humano.
    pub awaiting_input: bool,
    /// Agente caiu.
    pub failed: bool,
    /// Silenciado até este instante (ms desde a época). `None` = não silenciado.
    #[ts(type = "number | null")]
    pub muted_until: Option<crate::Millis>,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            awaiting_input: true,
            failed: true,
            muted_until: None,
        }
    }
}

impl NotificationSettings {
    /// Silenciado agora?
    pub fn muted(&self, now: crate::Millis) -> bool {
        self.muted_until.is_some_and(|until| until > now)
    }
}

/// Limites do barramento (`docs/07`), antes constantes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct BusSettings {
    pub guards: GuardConfig,
    /// Timeout de `aisense ask` sem `--timeout`.
    pub ask_default_secs: u32,
    /// Mensagens mais velhas que isto são apagadas (`docs/04`, "Retenção").
    pub retention_days: u32,
}

impl Default for BusSettings {
    fn default() -> Self {
        Self {
            guards: GuardConfig::default(),
            ask_default_secs: 300,
            retention_days: 90,
        }
    }
}

impl BusSettings {
    pub fn ask_default(&self) -> Duration {
        Duration::from_secs(u64::from(self.ask_default_secs))
    }

    pub fn retention_ms(&self) -> crate::Millis {
        i64::from(self.retention_days) * 24 * 60 * 60 * 1000
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AdvancedSettings {
    /// Vale na próxima subida (`AISENSE_LOG` ainda vence).
    pub log_level: LogLevel,
}

/// Um segredo de runtime: a variável de ambiente que o agente recebe. O valor mora no
/// keychain, com a chave [`SecretRef::key`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SecretRef {
    /// Runtime cujos agentes recebem o segredo.
    pub adapter_id: String,
    /// Nome da variável (`ANTHROPIC_API_KEY`...).
    pub env_name: String,
    /// Como a UI mostra o valor: `sk-…abcd` (`docs/11`). Nunca o valor inteiro.
    #[serde(default)]
    pub masked: String,
}

/// `sk-…abcd`: começo e fim, o bastante para reconhecer qual chave é. Valores curtos
/// não mostram nada — três + quatro caracteres de uma senha de oito seriam quase ela.
pub fn mask_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() < 16 {
        return "••••••••".to_owned();
    }
    let head: String = chars[..3].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

impl SecretRef {
    pub fn new(adapter_id: impl Into<String>, env_name: impl Into<String>, value: &str) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            env_name: env_name.into(),
            masked: mask_secret(value),
        }
    }

    /// Mesmo runtime e mesma variável (o `masked` não conta).
    pub fn same_slot(&self, other: &Self) -> bool {
        self.adapter_id == other.adapter_id && self.env_name == other.env_name
    }

    /// Nome da entrada no keychain.
    pub fn key(&self) -> String {
        format!("{}/{}", self.adapter_id, self.env_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct AppSettings {
    pub version: u32,
    /// O onboarding (T1) já foi concluído ou pulado: nunca mais aparece.
    pub onboarding_done: bool,
    pub appearance: AppearanceSettings,
    pub session: SessionSettings,
    pub notifications: NotificationSettings,
    pub bus: BusSettings,
    /// Atalho padrão → atalho escolhido (`⌘B` → `⌘⇧B`). Ausente = o padrão.
    pub shortcuts: std::collections::BTreeMap<String, String>,
    pub secrets: Vec<SecretRef>,
    pub advanced: AdvancedSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            onboarding_done: false,
            appearance: AppearanceSettings::default(),
            session: SessionSettings::default(),
            notifications: NotificationSettings::default(),
            bus: BusSettings::default(),
            shortcuts: std::collections::BTreeMap::new(),
            secrets: Vec::new(),
            advanced: AdvancedSettings::default(),
        }
    }
}

fn clamp<T: PartialOrd + Copy>(value: T, range: &std::ops::RangeInclusive<T>) -> T {
    if value < *range.start() {
        *range.start()
    } else if value > *range.end() {
        *range.end()
    } else {
        value
    }
}

impl AppSettings {
    /// Traz tudo para dentro das faixas válidas. Um arquivo editado à mão com
    /// `retention_days = 0` apagaria o histórico inteiro na subida; aqui vira 1.
    pub fn normalized(mut self) -> Self {
        self.version = SETTINGS_VERSION;
        let a = &mut self.appearance;
        a.terminal_font_size = clamp(a.terminal_font_size, &TERMINAL_FONT_SIZE_RANGE);
        a.terminal_font_family = a.terminal_font_family.trim().to_owned();
        let b = &mut self.bus;
        b.ask_default_secs = clamp(b.ask_default_secs, &ASK_DEFAULT_SECS_RANGE);
        b.retention_days = clamp(b.retention_days, &RETENTION_DAYS_RANGE);
        let g = &mut b.guards;
        g.per_agent_per_minute = g.per_agent_per_minute.max(1);
        g.max_reply_depth = g.max_reply_depth.max(1);
        g.max_identical = g.max_identical.max(1);
        g.team_per_hour = g.team_per_hour.max(1);
        self.shortcuts
            .retain(|from, to| !from.trim().is_empty() && !to.trim().is_empty() && from != to);
        self.secrets.retain(|s| valid_env_name(&s.env_name));
        // Um segredo por runtime e variável: o mais recente (o último da lista) vence.
        let mut kept: Vec<SecretRef> = Vec::new();
        for secret in self.secrets.drain(..).rev() {
            if !kept.iter().any(|k| k.same_slot(&secret)) {
                kept.push(secret);
            }
        }
        kept.sort();
        self.secrets = kept;
        self
    }
}

/// `[A-Za-z_][A-Za-z0-9_]*`, e nunca `AISENSE_*` (reservado à identidade, I7).
pub fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let head_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    head_ok
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name
            .to_ascii_uppercase()
            .starts_with(crate::agent::RESERVED_ENV_PREFIX)
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not serialize the settings: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl SettingsError {
    pub fn to_command_error(&self) -> crate::CommandError {
        crate::CommandError::new(
            "settings_write_failed",
            self.to_string(),
            Some(
                "Confira se a pasta de dados do AISENSE existe e tem permissão de escrita.".into(),
            ),
        )
    }
}

/// O que a carga encontrou. Um arquivo ilegível não impede o app de subir: vira os
/// padrões, e o original é guardado ao lado (`settings.json.corrupt`) para o usuário
/// recuperar o que quiser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub settings: AppSettings,
    /// Frase para o log/UI quando o arquivo existia e não deu para ler.
    pub warning: Option<String>,
}

/// Lê e grava o `settings.json`.
#[derive(Debug, Clone)]
pub struct SettingsFile {
    path: PathBuf,
}

impl SettingsFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Loaded {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Loaded {
                    settings: AppSettings::default(),
                    warning: None,
                }
            }
            Err(e) => {
                return Loaded {
                    settings: AppSettings::default(),
                    warning: Some(format!(
                        "não consegui ler {}: {e}; usando os padrões",
                        self.path.display()
                    )),
                }
            }
        };
        match serde_json::from_str::<AppSettings>(&text) {
            Ok(settings) => Loaded {
                settings: settings.normalized(),
                warning: None,
            },
            Err(e) => {
                let backup = self.path.with_extension("json.corrupt");
                let kept = fs::rename(&self.path, &backup).is_ok();
                let tail = if kept {
                    format!("o original foi guardado em {}", backup.display())
                } else {
                    "o original ficou onde estava".to_owned()
                };
                Loaded {
                    settings: AppSettings::default(),
                    warning: Some(format!(
                        "{} ilegível (linha {}): {e}; usando os padrões, {tail}",
                        self.path.display(),
                        e.line()
                    )),
                }
            }
        }
    }

    /// Grava de forma atômica e devolve o que foi gravado (já normalizado).
    pub fn save(&self, settings: AppSettings) -> Result<AppSettings, SettingsError> {
        let settings = settings.normalized();
        let text = serde_json::to_string_pretty(&settings)?;
        let write_err = |source| SettingsError::Write {
            path: self.path.clone(),
            source,
        };
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(write_err)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, format!("{text}\n")).map_err(write_err)?;
        fs::rename(&tmp, &self.path).map_err(write_err)?;
        Ok(settings)
    }
}

/// O que a tela de Configurações recebe: as preferências e onde as coisas moram.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SettingsView {
    pub settings: AppSettings,
    pub data_dir: String,
    pub settings_path: String,
    pub adapters_dir: String,
    /// O arquivo existia e não deu para ler (ver [`Loaded::warning`]).
    pub warning: Option<String>,
}

/// Porta para o keychain do SO. O core só conhece a interface; o app liga o keychain
/// de verdade, os testes usam memória.
pub trait SecretVault: Send + Sync + 'static {
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn delete(&self, key: &str) -> Result<(), String>;
}

/// Cofre em memória (testes; e o fallback quando o SO não tem keychain — que perde os
/// valores ao fechar, mas nunca os grava em disco simples).
#[derive(Default)]
pub struct MemoryVault(std::sync::Mutex<std::collections::HashMap<String, String>>);

impl MemoryVault {
    fn map(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, String>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SecretVault for MemoryVault {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.map().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.map().insert(key.to_owned(), value.to_owned());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        self.map().remove(key);
        Ok(())
    }
}

/// As variáveis que os agentes de um runtime recebem, lidas do cofre. Segredo sem
/// valor (apagado do keychain por fora) fica de fora, com aviso no log.
pub fn secret_env(
    settings: &AppSettings,
    vault: &dyn SecretVault,
    adapter_id: &str,
) -> Vec<(String, String)> {
    settings
        .secrets
        .iter()
        .filter(|s| s.adapter_id == adapter_id)
        .filter_map(|s| match vault.get(&s.key()) {
            Ok(Some(value)) => Some((s.env_name.clone(), value)),
            Ok(None) => {
                tracing::warn!(secret = %s.key(), "segredo sem valor no keychain");
                None
            }
            Err(error) => {
                tracing::warn!(secret = %s.key(), %error, "keychain não respondeu");
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn temp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn missing_file_loads_the_defaults_without_warning() {
        let dir = temp();
        let loaded = SettingsFile::new(dir.path().join("settings.json")).load();
        assert_eq!(loaded.settings, AppSettings::default());
        assert!(loaded.warning.is_none());
        assert!(!loaded.settings.onboarding_done);
    }

    #[test]
    fn saves_and_loads_back() {
        let dir = temp();
        let file = SettingsFile::new(dir.path().join("sub").join("settings.json"));
        let mut settings = AppSettings {
            onboarding_done: true,
            ..AppSettings::default()
        };
        settings.appearance.theme = ThemePreference::Dark;
        settings.session.last_team = Some(TeamId::from_raw("team_1"));
        settings.bus.guards.team_per_hour = 42;
        let saved = file.save(settings.clone()).unwrap();
        assert_eq!(saved, settings);
        assert_eq!(file.load().settings, settings);
        // Nada de temporário sobrando.
        assert!(!dir.path().join("sub").join("settings.json.tmp").exists());
    }

    #[test]
    fn an_old_file_without_new_sections_gets_their_defaults() {
        let dir = temp();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"onboardingDone": true, "futureField": 1, "bus": {"retentionDays": 30}}"#,
        )
        .unwrap();
        let loaded = SettingsFile::new(&path).load();
        assert!(loaded.warning.is_none());
        assert!(loaded.settings.onboarding_done);
        assert_eq!(loaded.settings.bus.retention_days, 30);
        assert_eq!(loaded.settings.bus.ask_default_secs, 300);
        assert_eq!(loaded.settings.bus.guards, GuardConfig::default());
        assert!(loaded.settings.notifications.enabled);
    }

    #[test]
    fn a_corrupt_file_falls_back_and_is_kept_aside() {
        let dir = temp();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        let loaded = SettingsFile::new(&path).load();
        assert_eq!(loaded.settings, AppSettings::default());
        let warning = loaded.warning.unwrap();
        assert!(warning.contains("settings.json.corrupt"), "{warning}");
        assert!(dir.path().join("settings.json.corrupt").exists());
    }

    #[test]
    fn values_out_of_range_are_clamped() {
        let mut settings = AppSettings::default();
        settings.bus.retention_days = 0;
        settings.bus.ask_default_secs = 99_999;
        settings.bus.guards.max_identical = 0;
        settings.appearance.terminal_font_size = 3;
        settings.shortcuts.insert("⌘B".into(), "⌘B".into());
        settings.shortcuts.insert("⌘I".into(), "⌘⇧I".into());
        let n = settings.normalized();
        assert_eq!(n.bus.retention_days, 1);
        assert_eq!(n.bus.ask_default_secs, 1_800);
        assert_eq!(n.bus.guards.max_identical, 1);
        assert_eq!(n.appearance.terminal_font_size, 10);
        assert_eq!(n.shortcuts.len(), 1);
    }

    #[test]
    fn env_names_are_validated_and_aisense_is_reserved() {
        assert!(valid_env_name("ANTHROPIC_API_KEY"));
        assert!(valid_env_name("_x1"));
        assert!(!valid_env_name("1ABC"));
        assert!(!valid_env_name("A-B"));
        assert!(!valid_env_name(""));
        assert!(!valid_env_name("AISENSE_TOKEN"));
        assert!(!valid_env_name("aisense_token"));
    }

    #[test]
    fn secret_env_reads_only_the_runtime_s_secrets_from_the_vault() {
        let vault = MemoryVault::default();
        let mut settings = AppSettings::default();
        let claude = SecretRef::new("claude", "ANTHROPIC_API_KEY", "sk-1");
        let codex = SecretRef::new("codex", "OPENAI_API_KEY", "sk-2");
        let gone = SecretRef::new("claude", "GONE", "x");
        vault.set(&claude.key(), "sk-1").unwrap();
        vault.set(&codex.key(), "sk-2").unwrap();
        settings.secrets = vec![claude, codex, gone];
        assert_eq!(
            secret_env(&settings, &vault, "claude"),
            vec![("ANTHROPIC_API_KEY".to_owned(), "sk-1".to_owned())]
        );
    }

    #[test]
    fn the_newest_secret_for_a_slot_wins_and_the_value_is_masked() {
        let settings = AppSettings {
            secrets: vec![
                SecretRef::new("claude", "KEY", "sk-ant-aaaaaaaaaaaa-1111"),
                SecretRef::new("claude", "KEY", "sk-ant-bbbbbbbbbbbb-2222"),
            ],
            ..AppSettings::default()
        };
        let n = settings.normalized();
        assert_eq!(n.secrets.len(), 1);
        assert_eq!(n.secrets[0].masked, "sk-…2222");
        assert_eq!(mask_secret("curta"), "••••••••");
    }

    #[test]
    fn mute_expires() {
        let n = NotificationSettings {
            muted_until: Some(1_000),
            ..NotificationSettings::default()
        };
        assert!(n.muted(999));
        assert!(!n.muted(1_000));
        assert!(!NotificationSettings::default().muted(0));
    }
}
