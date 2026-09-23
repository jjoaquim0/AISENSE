//! Modelos de equipe (`docs/09`, T3 passo 2): atalhos para não começar do zero.
//!
//! Cada agente do modelo traz uma lista de runtimes em ordem de preferência. Se o
//! preferido não está instalado, o próximo disponível entra no lugar — e o passo 3 do
//! assistente mostra a troca ("OpenCode não está instalado — @frontend usará Claude").

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agent::{AgentDraft, RestartPolicy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum TeamTemplate {
    Empty,
    DuoDev,
    FullSquad,
    Research,
    Operations,
}

/// Um agente que o modelo propõe, já com o runtime escolhido para esta máquina.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct PlannedAgent {
    pub draft: AgentDraft,
    /// O que o modelo queria usar. Diferente de `draft.adapter_id` quando houve troca.
    pub preferred_adapter_id: String,
    /// Nenhum runtime da lista de preferência está instalado; ficou o preferido, e o
    /// agente só vai subir depois que ele for instalado.
    pub unavailable: bool,
}

struct Blueprint {
    handle: &'static str,
    name: &'static str,
    role: &'static str,
    adapters: &'static [&'static str],
    restart_policy: RestartPolicy,
}

const CODERS: &[&str] = &["claude", "codex", "opencode", "gemini"];

const fn agent(
    handle: &'static str,
    name: &'static str,
    role: &'static str,
    adapters: &'static [&'static str],
) -> Blueprint {
    Blueprint {
        handle,
        name,
        role,
        adapters,
        restart_policy: RestartPolicy::OnCrash,
    }
}

const DUO_DEV: &[Blueprint] = &[
    agent(
        "dev",
        "Dev",
        "Implementa as tarefas da equipe. Escreve código, roda os testes e pede revisão ao @revisor antes de dar algo por pronto.",
        CODERS,
    ),
    agent(
        "revisor",
        "Revisor",
        "Revisor rigoroso. Lê cada mudança procurando bugs, casos esquecidos e testes faltando, e só aprova o que entende por inteiro.",
        CODERS,
    ),
];

const FULL_SQUAD: &[Blueprint] = &[
    agent(
        "arquiteto",
        "Arquiteto",
        "Define contratos entre as partes, quebra o trabalho em tarefas e decide quando uma mudança precisa de conversa antes de código.",
        CODERS,
    ),
    agent(
        "backend",
        "Backend",
        "Cuida da API, do banco e das integrações. Avisa o @frontend quando um contrato muda.",
        &["codex", "claude", "opencode", "gemini"],
    ),
    agent(
        "frontend",
        "Frontend",
        "Cuida da interface. Implementa telas a partir dos contratos combinados com o @backend.",
        &["opencode", "claude", "codex", "gemini"],
    ),
    agent(
        "revisor",
        "Revisor",
        "Revisa as mudanças de todos antes de irem para a branch principal.",
        CODERS,
    ),
];

const RESEARCH: &[Blueprint] = &[
    agent(
        "coordenador",
        "Coordenador",
        "Divide a pergunta em frentes, distribui entre os pesquisadores e cobra as entregas.",
        CODERS,
    ),
    agent(
        "pesquisador-1",
        "Pesquisador 1",
        "Investiga a frente recebida do @coordenador e entrega achados com fontes.",
        &["claude", "gemini", "codex", "opencode"],
    ),
    agent(
        "pesquisador-2",
        "Pesquisador 2",
        "Investiga a frente recebida do @coordenador e entrega achados com fontes.",
        &["gemini", "claude", "codex", "opencode"],
    ),
    agent(
        "pesquisador-3",
        "Pesquisador 3",
        "Investiga a frente recebida do @coordenador e entrega achados com fontes.",
        &["codex", "claude", "gemini", "opencode"],
    ),
    agent(
        "sintetizador",
        "Sintetizador",
        "Junta os achados dos pesquisadores num relatório único, apontando onde eles discordam.",
        CODERS,
    ),
];

const OPERATIONS: &[Blueprint] = &[
    Blueprint {
        handle: "monitor",
        name: "Monitor",
        role: "Terminal de longa duração para logs e comandos de observação.",
        adapters: &["shell"],
        // Monitor que cai deve voltar sempre, mesmo saindo "com sucesso".
        restart_policy: RestartPolicy::Always,
    },
    agent(
        "triagem",
        "Triagem",
        "Lê o que o @monitor encontra, separa ruído de incidente e descreve o próximo passo.",
        CODERS,
    ),
];

impl TeamTemplate {
    pub const ALL: [TeamTemplate; 5] = [
        Self::Empty,
        Self::DuoDev,
        Self::FullSquad,
        Self::Research,
        Self::Operations,
    ];

    fn blueprints(self) -> &'static [Blueprint] {
        match self {
            Self::Empty => &[],
            Self::DuoDev => DUO_DEV,
            Self::FullSquad => FULL_SQUAD,
            Self::Research => RESEARCH,
            Self::Operations => OPERATIONS,
        }
    }

    /// Os agentes do modelo, com runtime escolhido entre os que `is_available` aceita.
    pub fn plan(self, is_available: impl Fn(&str) -> bool) -> Vec<PlannedAgent> {
        self.blueprints()
            .iter()
            .map(|b| {
                let preferred = b.adapters.first().copied().unwrap_or("shell");
                let chosen = b.adapters.iter().copied().find(|id| is_available(id));
                PlannedAgent {
                    draft: AgentDraft {
                        handle: b.handle.to_owned(),
                        name: b.name.to_owned(),
                        role: b.role.to_owned(),
                        adapter_id: chosen.unwrap_or(preferred).to_owned(),
                        restart_policy: b.restart_policy,
                        // "▶ Iniciar equipe" sobe quem tem autostart; um modelo pronto é para subir.
                        autostart: true,
                        ..AgentDraft::default()
                    },
                    preferred_adapter_id: preferred.to_owned(),
                    unavailable: chosen.is_none(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::agent::{Agent, Handle};
    use crate::ids::TeamId;

    #[test]
    fn duo_dev_is_dev_and_revisor() {
        let plan = TeamTemplate::DuoDev.plan(|_| true);
        let handles: Vec<&str> = plan.iter().map(|p| p.draft.handle.as_str()).collect();
        assert_eq!(handles, vec!["dev", "revisor"]);
        assert!(plan
            .iter()
            .all(|p| p.draft.adapter_id == "claude" && !p.unavailable));
    }

    #[test]
    fn missing_runtime_falls_back_to_the_next_preference() {
        let plan = TeamTemplate::FullSquad.plan(|id| id == "claude");
        let frontend = plan.iter().find(|p| p.draft.handle == "frontend").unwrap();
        assert_eq!(frontend.preferred_adapter_id, "opencode");
        assert_eq!(frontend.draft.adapter_id, "claude");
        assert!(!frontend.unavailable);
    }

    #[test]
    fn nothing_installed_keeps_the_preferred_and_says_so() {
        let plan = TeamTemplate::DuoDev.plan(|_| false);
        assert!(plan
            .iter()
            .all(|p| p.unavailable && p.draft.adapter_id == "claude"));
    }

    #[test]
    fn every_template_builds_valid_unique_agents() {
        for template in TeamTemplate::ALL {
            let team = TeamId::new();
            let mut created: Vec<Agent> = Vec::new();
            for planned in template.plan(|_| true) {
                assert!(Handle::parse(&planned.draft.handle).is_ok(), "{template:?}");
                let agent = Agent::create(team.clone(), &planned.draft, &created, 1)
                    .unwrap_or_else(|e| panic!("{template:?}: {e}"));
                created.push(agent);
            }
            let mut colors: Vec<_> = created.iter().map(|a| a.color).collect();
            colors.dedup();
            assert_eq!(colors.len(), created.len(), "{template:?}: cores repetidas");
        }
    }

    #[test]
    fn operations_monitor_runs_on_the_shell_and_always_comes_back() {
        let plan = TeamTemplate::Operations.plan(|_| true);
        let monitor = &plan[0].draft;
        assert_eq!(monitor.adapter_id, "shell");
        assert_eq!(monitor.restart_policy, RestartPolicy::Always);
        assert!(
            plan.iter().all(|p| p.draft.autostart),
            "modelo pronto sobe com a equipe"
        );
    }
}
