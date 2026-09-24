//! Configurações (T9; F08-05): preferências em `settings.json`, segredos no keychain do
//! SO e o modo calibração do detector de estado.

use std::sync::{Arc, Mutex, PoisonError};

use aisense_core::adapter::{save_state_rules, AdapterCatalog, StateRules};
use aisense_core::settings::{
    secret_env, valid_env_name, AppSettings, SecretRef, SecretVault, SettingsFile, SettingsView,
};
use aisense_core::state::{calibrate, Calibration, TAIL_LINES};
use aisense_core::{AgentId, CommandError, DataDir, TeamId};
use tauri::{AppHandle, Emitter, Manager, State};

use super::agents::Supervisor;
use super::bus::Bus;
use super::runtimes::{Registry, ADAPTERS_CHANGED};

/// Payload: o `AppSettings` inteiro, depois de gravado.
pub const SETTINGS_CHANGED: &str = "settings:changed";

/// Nome do serviço no keychain.
const KEYCHAIN_SERVICE: &str = "dev.aisense.app";

pub struct SettingsHub {
    file: SettingsFile,
    current: Mutex<AppSettings>,
    vault: Arc<dyn SecretVault>,
    data: DataDir,
    /// Aviso da carga (arquivo ilegível), para a tela de Configurações mostrar.
    warning: Option<String>,
}

pub type Settings = Arc<SettingsHub>;

impl SettingsHub {
    pub fn get(&self) -> AppSettings {
        self.current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn save(&self, settings: AppSettings) -> Result<AppSettings, CommandError> {
        let saved = self.file.save(settings).map_err(|e| e.to_command_error())?;
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = saved.clone();
        Ok(saved)
    }

    /// Muda uma parte e grava.
    pub fn update(
        &self,
        change: impl FnOnce(&mut AppSettings),
    ) -> Result<AppSettings, CommandError> {
        let mut settings = self.get();
        change(&mut settings);
        self.save(settings)
    }

    /// As variáveis secretas de um runtime, lidas do keychain agora.
    pub fn secret_env(&self, adapter_id: &str) -> Vec<(String, String)> {
        secret_env(&self.get(), &*self.vault, adapter_id)
    }
}

/// Fechando o app: guarda quem estava rodando, para a opção "religar os agentes".
pub fn remember_running(settings: &SettingsHub, supervisor: &Supervisor) {
    let running = supervisor.running_agents();
    if let Err(error) = settings.update(|s| s.session.running_agents = running) {
        tracing::warn!(message = %error.message, "agentes rodando não gravados");
    }
}

/// Subida: religa quem estava rodando, se o usuário pediu, escalonado como o ▶ da equipe.
/// Agente que sumiu ou não sobe fica no log — a UI mostra o estado de cada um.
pub fn relaunch(settings: &SettingsHub, supervisor: &Supervisor) {
    let prefs = settings.get();
    if !prefs.session.relaunch_agents || prefs.session.running_agents.is_empty() {
        return;
    }
    let (supervisor, ids) = (supervisor.clone(), prefs.session.running_agents);
    tauri::async_runtime::spawn(async move {
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                tokio::time::sleep(aisense_core::supervisor::TEAM_START_STAGGER).await;
            }
            match supervisor.start(id).await {
                Ok(_) => tracing::info!(agent = %id, "agente religado"),
                Err(error) => tracing::warn!(agent = %id, %error, "agente não religado"),
            }
        }
    });
}

/// Keychain do SO (Keychain no macOS, Credential Manager no Windows, Secret Service no
/// Linux) pelo crate `keyring`.
struct Keychain;

impl Keychain {
    fn entry(key: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYCHAIN_SERVICE, key).map_err(|e| e.to_string())
    }
}

impl SecretVault for Keychain {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        match Self::entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        Self::entry(key)?
            .set_password(value)
            .map_err(|e| e.to_string())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Carrega as preferências antes de o resto do app subir: os limites do barramento e
/// a retenção dependem delas.
pub fn setup(data: &DataDir) -> Settings {
    let file = SettingsFile::new(data.settings());
    let loaded = file.load();
    if let Some(warning) = &loaded.warning {
        tracing::warn!(%warning, "preferências");
    }
    Arc::new(SettingsHub {
        file,
        current: Mutex::new(loaded.settings),
        vault: Arc::new(Keychain),
        data: data.clone(),
        warning: loaded.warning,
    })
}

#[tauri::command]
pub fn settings_get(settings: State<'_, Settings>) -> SettingsView {
    SettingsView {
        settings: settings.get(),
        data_dir: settings.data.root().display().to_string(),
        settings_path: settings.data.settings().display().to_string(),
        adapters_dir: settings.data.adapters().display().to_string(),
        warning: settings.warning.clone(),
    }
}

/// Aplica o que vale sem reiniciar (limites do barramento) e avisa a UI.
pub fn apply(app: &AppHandle, settings: &AppSettings) {
    if let Some(bus) = app.try_state::<Bus>() {
        bus.set_limits(settings.bus.guards.clone(), settings.bus.ask_default());
    }
    if let Err(error) = app.emit(SETTINGS_CHANGED, settings) {
        tracing::warn!(%error, "falha ao avisar a UI sobre preferências");
    }
}

/// Grava as preferências vindas do formulário. Os segredos e a última equipe não vêm
/// do formulário: continuam os que o app já tem.
#[tauri::command]
pub fn settings_save(
    app: AppHandle,
    settings: State<'_, Settings>,
    next: AppSettings,
) -> Result<AppSettings, CommandError> {
    let saved = settings.update(|current| {
        let secrets = std::mem::take(&mut current.secrets);
        let last_team = current.session.last_team.take();
        let running = std::mem::take(&mut current.session.running_agents);
        *current = next;
        current.secrets = secrets;
        current.session.last_team = last_team;
        current.session.running_agents = running;
    })?;
    apply(&app, &saved);
    Ok(saved)
}

/// Volta tudo ao padrão, menos os segredos (que só saem pelo botão deles) e o
/// onboarding (que não deve reaparecer).
#[tauri::command]
pub fn settings_reset(
    app: AppHandle,
    settings: State<'_, Settings>,
) -> Result<AppSettings, CommandError> {
    let saved = settings.update(|current| {
        *current = AppSettings {
            secrets: std::mem::take(&mut current.secrets),
            onboarding_done: current.onboarding_done,
            ..AppSettings::default()
        };
    })?;
    apply(&app, &saved);
    Ok(saved)
}

/// Onboarding concluído ou pulado (F08-04).
#[tauri::command]
pub fn settings_onboarding_done(
    app: AppHandle,
    settings: State<'_, Settings>,
) -> Result<AppSettings, CommandError> {
    let saved = settings.update(|s| s.onboarding_done = true)?;
    apply(&app, &saved);
    Ok(saved)
}

/// Equipe aberta agora (F08-06). Sem evento: só a próxima subida lê.
#[tauri::command]
pub fn settings_last_team(
    settings: State<'_, Settings>,
    team_id: Option<TeamId>,
) -> Result<(), CommandError> {
    if settings.get().session.last_team == team_id {
        return Ok(());
    }
    settings.update(|s| s.session.last_team = team_id)?;
    Ok(())
}

fn vault_error(message: String) -> CommandError {
    CommandError::new(
        "keychain_unavailable",
        format!("the OS keychain did not answer: {message}"),
        Some(
            "No Linux, o keychain é o Secret Service (GNOME Keyring ou KWallet): confira se \
             ele está rodando e desbloqueado. O segredo não foi gravado em disco."
                .into(),
        ),
    )
}

/// Guarda um segredo no keychain e registra o nome dele nas preferências.
#[tauri::command]
pub fn secret_set(
    app: AppHandle,
    settings: State<'_, Settings>,
    adapter_id: String,
    env_name: String,
    value: String,
) -> Result<AppSettings, CommandError> {
    let env_name = env_name.trim().to_owned();
    if !valid_env_name(&env_name) {
        return Err(CommandError::new(
            "invalid_env_name",
            format!("{env_name:?} is not a valid environment variable name"),
            Some("Use letras, dígitos e _, sem começar por dígito nem por AISENSE_.".into()),
        ));
    }
    if value.is_empty() {
        return Err(CommandError::new(
            "empty_secret",
            "the secret is empty",
            Some("Cole o valor da chave.".into()),
        ));
    }
    let secret = SecretRef::new(adapter_id, env_name, &value);
    settings
        .vault
        .set(&secret.key(), &value)
        .map_err(vault_error)?;
    let saved = settings.update(|s| s.secrets.push(secret))?;
    apply(&app, &saved);
    Ok(saved)
}

#[tauri::command]
pub fn secret_delete(
    app: AppHandle,
    settings: State<'_, Settings>,
    adapter_id: String,
    env_name: String,
) -> Result<AppSettings, CommandError> {
    let secret = SecretRef::new(adapter_id, env_name, "");
    settings.vault.delete(&secret.key()).map_err(vault_error)?;
    let saved = settings.update(|s| s.secrets.retain(|x| !x.same_slot(&secret)))?;
    apply(&app, &saved);
    Ok(saved)
}

/// A tela atual do agente, sem ANSI — o que os regex enxergam.
#[tauri::command]
pub fn calibration_screen(supervisor: State<'_, Supervisor>, agent_id: AgentId) -> String {
    supervisor
        .previews(std::slice::from_ref(&agent_id), TAIL_LINES)
        .into_iter()
        .next()
        .map(|p| p.lines.join("\n"))
        .unwrap_or_default()
}

/// O que o detector decidiria com estas regras para esta tela.
#[tauri::command]
pub fn calibration_test(rules: StateRules, screen: String) -> Calibration {
    calibrate(&rules, &screen)
}

/// Grava as regras no adaptador e já as aplica às sessões vivas, sem reiniciar nada.
/// Devolve o arquivo gravado.
#[tauri::command]
pub async fn calibration_apply(
    app: AppHandle,
    registry: State<'_, Registry>,
    supervisor: State<'_, Supervisor>,
    settings: State<'_, Settings>,
    adapter_id: String,
    rules: StateRules,
) -> Result<String, CommandError> {
    let adapter = registry.adapter(&adapter_id).ok_or_else(|| {
        CommandError::new(
            "unknown_adapter",
            format!("runtime {adapter_id:?} is not configured"),
            None,
        )
    })?;
    let dir = settings.data.adapters();
    let path = save_state_rules(&adapter, &rules, &dir).map_err(|e| e.to_command_error())?;
    // Recarrega já, sem esperar o observador da pasta (que pode estar desligado).
    registry.set_catalog(AdapterCatalog::load(&dir));
    let updated = supervisor.refresh_state_rules();
    tracing::info!(adapter = %adapter_id, updated, "regras de estado calibradas");
    if let Err(error) = app.emit(ADAPTERS_CHANGED, ()) {
        tracing::warn!(%error, "falha ao avisar a UI sobre adaptadores novos");
    }
    Ok(path.display().to_string())
}

/// Grava um diagnóstico (versão, SO, preferências sem segredos, adaptadores com
/// problema) para anexar a um relato de bug. Devolve o caminho.
#[tauri::command]
pub fn diagnostics_export(
    settings: State<'_, Settings>,
    registry: State<'_, Registry>,
) -> Result<String, CommandError> {
    let catalog = registry.catalog();
    let mut prefs = settings.get();
    // Só os nomes, e mesmo assim sem o runtime: basta saber quantos há.
    let secrets = prefs.secrets.len();
    prefs.secrets.clear();
    let report = serde_json::json!({
        "app": aisense_core::AppInfo::current(),
        "dataDir": settings.data.root().display().to_string(),
        "settings": prefs,
        "secretsConfigured": secrets,
        "adapters": catalog.adapters().map(|a| &a.id).collect::<Vec<_>>(),
        "adapterProblems": catalog.problems().iter().map(ToString::to_string).collect::<Vec<_>>(),
        "generatedAt": aisense_core::now_ms(),
    });
    let path = settings
        .data
        .logs()
        .join(format!("diagnostico-{}.json", aisense_core::now_ms()));
    let write = |e: std::io::Error| {
        CommandError::new(
            "diagnostics_failed",
            format!("could not write {}: {e}", path.display()),
            Some("Confira as permissões da pasta de dados.".into()),
        )
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(write)?;
    }
    let text = serde_json::to_string_pretty(&report).unwrap_or_default();
    std::fs::write(&path, text).map_err(write)?;
    Ok(path.display().to_string())
}
