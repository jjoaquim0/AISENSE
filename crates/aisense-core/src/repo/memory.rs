//! Implementação em memória das portas, com as mesmas garantias do SQLite
//! (unicidade de handle, cascata). Serve aos testes do domínio e do supervisor.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use super::{
    dedup_agent_skills, skill_origin, AgentRepository, AgentSkill, RepoError, RepoResult,
    SessionRecord, SessionRepository, SkillRecord, SkillRepository, TeamFilter, TeamRepository,
    SESSIONS_KEPT_PER_AGENT,
};
use crate::agent::{Agent, Handle};
use crate::ids::{AgentId, SessionId, SkillId, TeamId};
use crate::skill::Skill;
use crate::team::Team;
use crate::time::Millis;

#[derive(Default)]
struct Inner {
    teams: BTreeMap<String, Team>,
    agents: BTreeMap<String, Agent>,
    /// Em ordem de início.
    sessions: Vec<SessionRecord>,
    /// Por `slug`.
    skills: BTreeMap<String, SkillRecord>,
    /// Por agente, na ordem de injeção.
    agent_skills: BTreeMap<String, Vec<AgentSkill>>,
}

#[derive(Default)]
pub struct InMemoryStore {
    inner: Mutex<Inner>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // Envenenado = pânico em outro teste; os dados continuam utilizáveis.
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl Inner {
    fn handle_taken(&self, agent: &Agent) -> bool {
        self.agents
            .values()
            .any(|a| a.team_id == agent.team_id && a.handle == agent.handle && a.id != agent.id)
    }
}

impl TeamRepository for InMemoryStore {
    async fn create_team(&self, team: &Team) -> RepoResult<()> {
        let mut inner = self.lock();
        if inner.teams.contains_key(team.id.as_str()) {
            return Err(RepoError::AlreadyExists(team.id.to_string()));
        }
        inner
            .teams
            .insert(team.id.as_str().to_owned(), team.clone());
        Ok(())
    }

    async fn get_team(&self, id: &TeamId) -> RepoResult<Option<Team>> {
        Ok(self.lock().teams.get(id.as_str()).cloned())
    }

    async fn list_teams(&self, filter: TeamFilter) -> RepoResult<Vec<Team>> {
        let mut teams: Vec<Team> = self
            .lock()
            .teams
            .values()
            .filter(|t| filter.include_archived || !t.is_archived())
            .cloned()
            .collect();
        teams.sort_by(|a, b| (a.created_at, a.id.as_str()).cmp(&(b.created_at, b.id.as_str())));
        Ok(teams)
    }

    async fn update_team(&self, team: &Team) -> RepoResult<()> {
        let mut inner = self.lock();
        match inner.teams.get_mut(team.id.as_str()) {
            Some(slot) => {
                *slot = team.clone();
                Ok(())
            }
            None => Err(RepoError::TeamNotFound(team.id.clone())),
        }
    }

    async fn set_team_archived(&self, id: &TeamId, archived_at: Option<Millis>) -> RepoResult<()> {
        let mut inner = self.lock();
        let team = inner
            .teams
            .get_mut(id.as_str())
            .ok_or_else(|| RepoError::TeamNotFound(id.clone()))?;
        team.archived_at = archived_at;
        Ok(())
    }

    async fn delete_team(&self, id: &TeamId) -> RepoResult<()> {
        let mut inner = self.lock();
        if inner.teams.remove(id.as_str()).is_none() {
            return Err(RepoError::TeamNotFound(id.clone()));
        }
        inner.agents.retain(|_, a| a.team_id != *id);
        let alive_agents: Vec<String> = inner.agents.keys().cloned().collect();
        inner
            .agent_skills
            .retain(|agent, _| alive_agents.contains(agent));
        let agents = &inner.agents;
        let alive: Vec<SessionRecord> = inner
            .sessions
            .iter()
            .filter(|s| agents.contains_key(s.agent_id.as_str()))
            .cloned()
            .collect();
        inner.sessions = alive;
        Ok(())
    }
}

impl AgentRepository for InMemoryStore {
    async fn create_agent(&self, agent: &Agent) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.teams.contains_key(agent.team_id.as_str()) {
            return Err(RepoError::TeamNotFound(agent.team_id.clone()));
        }
        if inner.agents.contains_key(agent.id.as_str()) {
            return Err(RepoError::AlreadyExists(agent.id.to_string()));
        }
        if inner.handle_taken(agent) {
            return Err(RepoError::DuplicateHandle(agent.handle.as_str().to_owned()));
        }
        inner
            .agents
            .insert(agent.id.as_str().to_owned(), agent.clone());
        Ok(())
    }

    async fn get_agent(&self, id: &AgentId) -> RepoResult<Option<Agent>> {
        Ok(self.lock().agents.get(id.as_str()).cloned())
    }

    async fn find_agent_by_handle(
        &self,
        team_id: &TeamId,
        handle: &Handle,
    ) -> RepoResult<Option<Agent>> {
        Ok(self
            .lock()
            .agents
            .values()
            .find(|a| a.team_id == *team_id && a.handle == *handle)
            .cloned())
    }

    async fn list_agents(&self, team_id: &TeamId) -> RepoResult<Vec<Agent>> {
        let mut agents: Vec<Agent> = self
            .lock()
            .agents
            .values()
            .filter(|a| a.team_id == *team_id)
            .cloned()
            .collect();
        agents.sort_by(|a, b| {
            (a.position, a.created_at, a.id.as_str()).cmp(&(
                b.position,
                b.created_at,
                b.id.as_str(),
            ))
        });
        Ok(agents)
    }

    async fn update_agent(&self, agent: &Agent) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.agents.contains_key(agent.id.as_str()) {
            return Err(RepoError::AgentNotFound(agent.id.clone()));
        }
        if inner.handle_taken(agent) {
            return Err(RepoError::DuplicateHandle(agent.handle.as_str().to_owned()));
        }
        inner
            .agents
            .insert(agent.id.as_str().to_owned(), agent.clone());
        Ok(())
    }

    async fn delete_agent(&self, id: &AgentId) -> RepoResult<()> {
        let mut inner = self.lock();
        match inner.agents.remove(id.as_str()) {
            Some(_) => {
                inner.sessions.retain(|s| s.agent_id != *id);
                inner.agent_skills.remove(id.as_str());
                Ok(())
            }
            None => Err(RepoError::AgentNotFound(id.clone())),
        }
    }
}

impl SessionRepository for InMemoryStore {
    async fn start_session(&self, session: &SessionRecord) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.agents.contains_key(session.agent_id.as_str()) {
            return Err(RepoError::AgentNotFound(session.agent_id.clone()));
        }
        if inner.sessions.iter().any(|s| s.id == session.id) {
            return Err(RepoError::AlreadyExists(session.id.to_string()));
        }
        inner.sessions.push(session.clone());
        let of_agent = inner
            .sessions
            .iter()
            .filter(|s| s.agent_id == session.agent_id)
            .count();
        let mut excess = of_agent.saturating_sub(SESSIONS_KEPT_PER_AGENT);
        inner.sessions.retain(|s| {
            if excess > 0 && s.agent_id == session.agent_id {
                excess -= 1;
                return false;
            }
            true
        });
        Ok(())
    }

    async fn end_session(
        &self,
        id: &SessionId,
        ended_at: Millis,
        exit_code: Option<i32>,
    ) -> RepoResult<()> {
        if let Some(session) = self.lock().sessions.iter_mut().find(|s| s.id == *id) {
            session.ended_at = Some(ended_at);
            session.exit_code = exit_code;
        }
        Ok(())
    }

    async fn list_sessions(
        &self,
        agent_id: &AgentId,
        limit: usize,
    ) -> RepoResult<Vec<SessionRecord>> {
        Ok(self
            .lock()
            .sessions
            .iter()
            .rev()
            .filter(|s| s.agent_id == *agent_id)
            .take(limit)
            .cloned()
            .collect())
    }
}

impl SkillRepository for InMemoryStore {
    async fn sync_skills(&self, skills: &[Skill], now: Millis) -> RepoResult<Vec<SkillRecord>> {
        let mut inner = self.lock();
        for skill in skills {
            let (source, path) = skill_origin(skill);
            match inner.skills.get_mut(&skill.name) {
                Some(record) if record.matches(skill) => {}
                Some(record) => {
                    record.description.clone_from(&skill.description);
                    record.version.clone_from(&skill.version);
                    source.clone_into(&mut record.source);
                    record.path = path;
                    record.targets.clone_from(&skill.targets);
                    record.updated_at = now;
                }
                None => {
                    let record = SkillRecord {
                        id: SkillId::new(),
                        slug: skill.name.clone(),
                        description: skill.description.clone(),
                        version: skill.version.clone(),
                        source: source.to_owned(),
                        path,
                        targets: skill.targets.clone(),
                        created_at: now,
                        updated_at: now,
                    };
                    inner.skills.insert(skill.name.clone(), record);
                }
            }
        }
        Ok(inner.skills.values().cloned().collect())
    }

    async fn list_skills(&self) -> RepoResult<Vec<SkillRecord>> {
        Ok(self.lock().skills.values().cloned().collect())
    }

    async fn agent_skills(&self, agent_id: &AgentId) -> RepoResult<Vec<AgentSkill>> {
        Ok(self
            .lock()
            .agent_skills
            .get(agent_id.as_str())
            .cloned()
            .unwrap_or_default())
    }

    async fn set_agent_skills(&self, agent_id: &AgentId, skills: &[AgentSkill]) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.agents.contains_key(agent_id.as_str()) {
            return Err(RepoError::AgentNotFound(agent_id.clone()));
        }
        if let Some(missing) = skills
            .iter()
            .find(|s| !inner.skills.values().any(|r| r.id == s.skill_id))
        {
            return Err(RepoError::SkillNotFound(missing.skill_id.clone()));
        }
        inner
            .agent_skills
            .insert(agent_id.as_str().to_owned(), dedup_agent_skills(skills));
        Ok(())
    }

    async fn skill_users(&self, skill_id: &SkillId) -> RepoResult<Vec<AgentId>> {
        Ok(self
            .lock()
            .agent_skills
            .iter()
            .filter(|(_, list)| list.iter().any(|s| s.skill_id == *skill_id))
            .map(|(agent, _)| AgentId::from_raw(agent.clone()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::agent::AgentDraft;
    use crate::team::TeamDraft;

    fn team(name: &str, now: Millis) -> Team {
        Team::create(
            &TeamDraft {
                name: name.into(),
                workdir: "/tmp".into(),
                ..TeamDraft::default()
            },
            now,
        )
        .unwrap()
    }

    fn agent(team: &Team, handle: &str) -> Agent {
        let d = AgentDraft {
            handle: handle.into(),
            name: handle.into(),
            adapter_id: "shell".into(),
            ..AgentDraft::default()
        };
        Agent::create(team.id.clone(), &d, &[], 0).unwrap()
    }

    #[tokio::test]
    async fn deleting_a_team_cascades_to_its_agents() {
        let store = InMemoryStore::new();
        let (a, b) = (team("A", 1), team("B", 2));
        store.create_team(&a).await.unwrap();
        store.create_team(&b).await.unwrap();
        store.create_agent(&agent(&a, "backend")).await.unwrap();
        store.create_agent(&agent(&b, "backend")).await.unwrap();

        store.delete_team(&a.id).await.unwrap();
        assert!(store.list_agents(&a.id).await.unwrap().is_empty());
        assert_eq!(store.list_agents(&b.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn the_port_enforces_unique_handles_too() {
        let store = InMemoryStore::new();
        let t = team("A", 1);
        store.create_team(&t).await.unwrap();
        store.create_agent(&agent(&t, "backend")).await.unwrap();
        let err = store.create_agent(&agent(&t, "backend")).await.unwrap_err();
        assert!(matches!(err, RepoError::DuplicateHandle(h) if h == "backend"));
    }

    #[tokio::test]
    async fn agents_need_an_existing_team() {
        let store = InMemoryStore::new();
        let orphan = agent(&team("ghost", 1), "backend");
        assert!(matches!(
            store.create_agent(&orphan).await,
            Err(RepoError::TeamNotFound(_))
        ));
    }

    #[tokio::test]
    async fn archived_teams_are_hidden_by_default() {
        let store = InMemoryStore::new();
        let t = team("A", 1);
        store.create_team(&t).await.unwrap();
        store.create_agent(&agent(&t, "backend")).await.unwrap();
        store.set_team_archived(&t.id, Some(10)).await.unwrap();

        assert!(store
            .list_teams(TeamFilter::default())
            .await
            .unwrap()
            .is_empty());
        let all = store
            .list_teams(TeamFilter {
                include_archived: true,
            })
            .await
            .unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(
            store.list_agents(&t.id).await.unwrap().len(),
            1,
            "archiving keeps agents"
        );
    }
}
