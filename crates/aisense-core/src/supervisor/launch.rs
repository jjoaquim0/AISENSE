//! Como um agente vira um processo: comando, argumentos, diretório e ambiente
//! (`docs/02`, fluxo 1, passos 1 e 5; `docs/05`, "Ambiente injetado em TODO agente").
//!
//! Função pura: nada aqui abre processo, lê disco além de procurar o executável, ou
//! guarda estado. É o que torna a montagem testável sem PTY.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::adapter::{resolve_command, which, Adapter, AGENT_COMMAND_PLACEHOLDER};
use crate::agent::{Agent, RESERVED_ENV_PREFIX};
use crate::team::Team;

/// O que o supervisor sabe e o agente não: onde fica o barramento, os sidecars etc.
#[derive(Debug, Clone, Default)]
pub struct LaunchContext {
    /// Caminho do socket (ou named pipe) do barramento.
    pub socket: String,
    /// Pasta com `aisense` e `aisense-mcp`, posta na frente do `PATH`.
    pub sidecar_dir: Option<PathBuf>,
    /// `PATH` herdado (normalmente o do próprio app).
    pub inherited_path: Option<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// Caminho completo do executável, já resolvido no `PATH`.
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Ordem importa: o que vem depois vence (adaptador → agente → AISENSE).
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("`{command}` was not found in PATH")]
    NotInstalled {
        command: String,
        install_hint: Option<String>,
    },
    #[error("the custom runtime needs a command: put it as the agent's first argument")]
    MissingCustomCommand,
}

/// Identidade da execução, injetada como `AISENSE_*`.
#[derive(Debug, Clone)]
pub struct LaunchIdentity<'a> {
    pub token: &'a str,
}

pub fn build_launch(
    agent: &Agent,
    team: &Team,
    adapter: &Adapter,
    identity: &LaunchIdentity<'_>,
    context: &LaunchContext,
    // Já decidido pelas bancadas (`bench::prepare_workdir`): a do agente ou a da equipe.
    cwd: &Path,
) -> Result<LaunchPlan, LaunchError> {
    let (command, mut args) = if adapter.command == AGENT_COMMAND_PLACEHOLDER {
        let (command, rest) = agent
            .args
            .split_first()
            .ok_or(LaunchError::MissingCustomCommand)?;
        (command.clone(), rest.to_vec())
    } else {
        let mut args = adapter.args.clone();
        if let (Some(flag), Some(model)) = (&adapter.capabilities.model_flag, &agent.model) {
            args.push(flag.clone());
            args.push(model.clone());
        }
        args.extend(agent.args.iter().cloned());
        (resolve_command(&adapter.command), args)
    };
    args.retain(|a| !a.is_empty());

    let program = resolve_program(&command, &context.inherited_path).ok_or_else(|| {
        LaunchError::NotInstalled {
            command: command.clone(),
            install_hint: adapter.install_hint.clone(),
        }
    })?;

    Ok(LaunchPlan {
        program,
        args,
        env: environment(agent, team, adapter, identity, context, cwd),
        cwd: cwd.to_path_buf(),
    })
}

/// Procura no `PATH` que o agente vai receber — não no do app —, para achar o mesmo
/// executável que um `which` no terminal do agente acharia.
fn resolve_program(command: &str, inherited_path: &Option<OsString>) -> Option<PathBuf> {
    let candidate = Path::new(command);
    if candidate.is_absolute() && candidate.is_file() {
        return Some(candidate.to_path_buf());
    }
    match inherited_path {
        Some(path) => crate::adapter::which_in_path(command, path),
        None => which(command),
    }
}

fn environment(
    agent: &Agent,
    team: &Team,
    adapter: &Adapter,
    identity: &LaunchIdentity<'_>,
    context: &LaunchContext,
    cwd: &Path,
) -> Vec<(String, String)> {
    // BTreeMap para a ordem ser estável (testes, logs), e para o que vem depois
    // sobrescrever o que veio antes.
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    env.extend(adapter.env.clone());
    // O agente já foi validado sem `AISENSE_*` (I7), mas a regra vale aqui também:
    // esta é a última barreira antes do processo.
    env.extend(
        agent
            .env
            .iter()
            .filter(|(k, _)| !k.to_ascii_uppercase().starts_with(RESERVED_ENV_PREFIX))
            .map(|(k, v)| (k.clone(), v.clone())),
    );
    let aisense = [
        ("AISENSE_SOCKET", context.socket.clone()),
        ("AISENSE_TOKEN", identity.token.to_owned()),
        ("AISENSE_AGENT_ID", agent.id.as_str().to_owned()),
        ("AISENSE_AGENT_HANDLE", agent.handle.as_str().to_owned()),
        ("AISENSE_TEAM_ID", team.id.as_str().to_owned()),
        ("AISENSE_TEAM_NAME", team.name.clone()),
        ("AISENSE_WORKDIR", cwd.display().to_string()),
    ];
    env.extend(aisense.into_iter().map(|(k, v)| (k.to_owned(), v)));

    if let Some(path) = path_with_sidecars(context) {
        env.insert(path_key(&env), path);
    }
    env.into_iter().collect()
}

/// No Windows a variável se chama `Path` na maioria das máquinas; o valor é o mesmo.
/// Se o agente já trouxe uma, respeitamos a grafia dele.
fn path_key(env: &BTreeMap<String, String>) -> String {
    env.keys()
        .find(|k| k.eq_ignore_ascii_case("PATH"))
        .cloned()
        .unwrap_or_else(|| "PATH".to_owned())
}

fn path_with_sidecars(context: &LaunchContext) -> Option<String> {
    let sidecars = context.sidecar_dir.as_ref()?;
    let mut dirs = vec![sidecars.clone()];
    if let Some(inherited) = &context.inherited_path {
        dirs.extend(std::env::split_paths(inherited));
    }
    std::env::join_paths(dirs)
        .ok()
        .and_then(|p| p.into_string().ok())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::adapter::{parse_adapter, AdapterSource};
    use crate::agent::AgentDraft;
    use crate::team::TeamDraft;

    /// Um executável que existe em qualquer máquina de CI.
    fn real_program() -> &'static str {
        if cfg!(windows) {
            "cmd"
        } else {
            "sh"
        }
    }

    fn fixture(adapter_toml: &str, draft: AgentDraft) -> (Agent, Team, Adapter) {
        let team = Team::create(
            &TeamDraft {
                name: "Squad Produto".into(),
                workdir: std::env::temp_dir().display().to_string(),
                ..TeamDraft::default()
            },
            1,
        )
        .unwrap();
        let agent = Agent::create(team.id.clone(), &draft, &[], 1).unwrap();
        let adapter = parse_adapter(adapter_toml, "t.toml", AdapterSource::Builtin).unwrap();
        (agent, team, adapter)
    }

    fn draft() -> AgentDraft {
        AgentDraft {
            handle: "backend".into(),
            name: "Backend".into(),
            adapter_id: "x".into(),
            ..AgentDraft::default()
        }
    }

    fn adapter_running(program: &str) -> String {
        format!(
            "id = \"x\"\nname = \"X\"\ncommand = \"{program}\"\nargs = [\"--base\"]\n\
             [capabilities]\nmodel_flag = \"--model\"\n[env]\nFROM_ADAPTER = \"1\"\nSHARED = \"adapter\"\n"
        )
    }

    fn context() -> LaunchContext {
        LaunchContext {
            socket: "/run/aisense.sock".into(),
            sidecar_dir: Some(PathBuf::from("/opt/aisense/bin")),
            inherited_path: std::env::var_os("PATH"),
        }
    }

    fn launch(agent: &Agent, team: &Team, adapter: &Adapter) -> Result<LaunchPlan, LaunchError> {
        build_launch(
            agent,
            team,
            adapter,
            &LaunchIdentity { token: "tok" },
            &context(),
            Path::new(agent.workdir.as_deref().unwrap_or(&team.workdir)),
        )
    }

    fn env_of(plan: &LaunchPlan, key: &str) -> Option<String> {
        plan.env
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone())
    }

    #[test]
    fn identity_and_bus_reach_the_process() {
        let (agent, team, adapter) = fixture(&adapter_running(real_program()), draft());
        let plan = launch(&agent, &team, &adapter).unwrap();
        assert_eq!(env_of(&plan, "AISENSE_TOKEN").as_deref(), Some("tok"));
        assert_eq!(
            env_of(&plan, "AISENSE_AGENT_HANDLE").as_deref(),
            Some("backend")
        );
        assert_eq!(
            env_of(&plan, "AISENSE_AGENT_ID"),
            Some(agent.id.to_string())
        );
        assert_eq!(env_of(&plan, "AISENSE_TEAM_ID"), Some(team.id.to_string()));
        assert_eq!(
            env_of(&plan, "AISENSE_TEAM_NAME").as_deref(),
            Some("Squad Produto")
        );
        assert_eq!(
            env_of(&plan, "AISENSE_SOCKET").as_deref(),
            Some("/run/aisense.sock")
        );
        assert_eq!(env_of(&plan, "FROM_ADAPTER").as_deref(), Some("1"));
    }

    #[test]
    fn agent_env_overrides_adapter_but_never_aisense() {
        let mut d = draft();
        d.env = BTreeMap::from([("SHARED".into(), "agent".into())]);
        let (mut agent, team, adapter) = fixture(&adapter_running(real_program()), d);
        // Dado editado à mão no banco, fora da validação do core.
        agent.env.insert("AISENSE_TOKEN".into(), "roubado".into());
        let plan = launch(&agent, &team, &adapter).unwrap();
        assert_eq!(env_of(&plan, "SHARED").as_deref(), Some("agent"));
        assert_eq!(env_of(&plan, "AISENSE_TOKEN").as_deref(), Some("tok"));
    }

    #[test]
    fn sidecars_come_first_in_path() {
        let (agent, team, adapter) = fixture(&adapter_running(real_program()), draft());
        let plan = launch(&agent, &team, &adapter).unwrap();
        let path = env_of(&plan, "PATH").unwrap();
        let first = std::env::split_paths(&path).next().unwrap();
        assert_eq!(first, PathBuf::from("/opt/aisense/bin"));
    }

    #[test]
    fn args_are_adapter_then_model_then_agent() {
        let mut d = draft();
        d.model = Some("opus".into());
        d.args = vec!["--verbose".into()];
        let (agent, team, adapter) = fixture(&adapter_running(real_program()), d);
        let plan = launch(&agent, &team, &adapter).unwrap();
        assert_eq!(plan.args, vec!["--base", "--model", "opus", "--verbose"]);
    }

    #[test]
    fn the_given_workdir_is_the_cwd_and_aisense_workdir() {
        let (agent, team, adapter) = fixture(&adapter_running(real_program()), draft());
        let bench = std::env::temp_dir().join("bancada");
        let plan = build_launch(
            &agent,
            &team,
            &adapter,
            &LaunchIdentity { token: "tok" },
            &context(),
            &bench,
        )
        .unwrap();
        assert_eq!(plan.cwd, bench);
        assert_eq!(
            env_of(&plan, "AISENSE_WORKDIR"),
            Some(bench.display().to_string())
        );
    }

    #[test]
    fn missing_runtime_carries_the_install_hint() {
        let toml = "id = \"x\"\nname = \"X\"\ncommand = \"aisense-no-such-cli-42\"\n\
                    install_hint = \"npm i -g x\"\n";
        let (agent, team, adapter) = fixture(toml, draft());
        assert_eq!(
            launch(&agent, &team, &adapter).unwrap_err(),
            LaunchError::NotInstalled {
                command: "aisense-no-such-cli-42".into(),
                install_hint: Some("npm i -g x".into()),
            }
        );
    }

    #[test]
    fn custom_runtime_takes_the_command_from_the_agent() {
        let custom = "id = \"custom\"\nname = \"C\"\ncommand = \"$AGENT_COMMAND\"\n";
        let mut d = draft();
        d.args = vec![real_program().into(), "--flag".into()];
        let (agent, team, adapter) = fixture(custom, d);
        let plan = launch(&agent, &team, &adapter).unwrap();
        let name = plan
            .program
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        assert_eq!(name, real_program());
        assert_eq!(plan.args, vec!["--flag"]);

        let (agent, team, adapter) = fixture(custom, draft());
        assert_eq!(
            launch(&agent, &team, &adapter).unwrap_err(),
            LaunchError::MissingCustomCommand
        );
    }
}
