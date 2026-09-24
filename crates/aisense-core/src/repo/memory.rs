//! Implementação em memória das portas, com as mesmas garantias do SQLite
//! (unicidade de handle, cascata). Serve aos testes do domínio e do supervisor.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use super::{
    dedup_agent_skills, skill_origin, AgentRepository, AgentSkill, RepoError, RepoResult,
    SessionRecord, SessionRepository, SkillRecord, SkillRepository, TeamFilter, TeamRepository,
    TokenRecord, TokenRepository, SESSIONS_KEPT_PER_AGENT,
};
use crate::agent::{Agent, Handle};
use crate::board::{
    Activity, Actor, Automation, Board, BoardRepository, Card, CardQuery, CardWrite, Column,
    Comment, WriteGuard,
};
use crate::bus::{BusRepository, Channel, Delivery, DeliveryState, InboxItem, InboxQuery, Message};
use crate::ids::{AgentId, BoardId, CardId, ColumnId, MessageId, SessionId, SkillId, TeamId};
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
    channels: Vec<Channel>,
    /// `AISENSE_TOKEN` → dono.
    tokens: BTreeMap<String, TokenRecord>,
    /// Por id: ULID monotônico, então a ordem da chave é a ordem de criação.
    messages: BTreeMap<String, Message>,
    /// Por (mensagem, agente).
    deliveries: BTreeMap<(String, String), Delivery>,
    /// Por equipe.
    boards: BTreeMap<String, Board>,
    columns: Vec<Column>,
    cards: BTreeMap<String, Card>,
    /// (cartão, depende de).
    dependencies: std::collections::BTreeSet<(String, String)>,
    comments: Vec<Comment>,
    activity: Vec<Activity>,
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

impl Inner {
    /// Cartões, dependências, comentários e histórico de cartões que não existem mais.
    fn forget_orphans(&mut self) {
        let cards = &self.cards;
        self.dependencies
            .retain(|(t, d)| cards.contains_key(t) && cards.contains_key(d));
        self.comments
            .retain(|c| cards.contains_key(c.card_id.as_str()));
        self.activity
            .retain(|a| cards.contains_key(a.card_id.as_str()));
    }

    fn card_fits(&self, card: &Card, guard: &WriteGuard) -> bool {
        let others = || {
            self.cards.values().filter(move |c| {
                c.column_id == card.column_id && c.archived_at.is_none() && c.id != card.id
            })
        };
        let total_ok = guard
            .wip_limit
            .is_none_or(|limit| others().count() < limit as usize);
        let agent_ok = match (&card.assignee, guard.wip_per_agent) {
            (Some(agent), Some(limit)) => {
                others()
                    .filter(|c| c.assignee.as_ref() == Some(agent))
                    .count()
                    < limit as usize
            }
            _ => true,
        };
        total_ok && agent_ok
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
        // Cascata do quadro.
        if let Some(board) = inner.boards.remove(id.as_str()) {
            inner.columns.retain(|c| c.board_id != board.id);
        }
        inner.cards.retain(|_, c| c.team_id != *id);
        inner.forget_orphans();
        // Cascata do barramento, como as FKs do SQLite.
        inner.channels.retain(|c| c.team_id != *id);
        inner.messages.retain(|_, m| m.team_id != *id);
        let (messages, agents) = (&inner.messages, &inner.agents);
        let kept: BTreeMap<(String, String), Delivery> = inner
            .deliveries
            .iter()
            .filter(|((m, a), _)| messages.contains_key(m) && agents.contains_key(a))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        inner.deliveries = kept;
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
                inner
                    .deliveries
                    .retain(|(_, agent), _| agent != id.as_str());
                // ON DELETE SET NULL do quadro.
                for card in inner.cards.values_mut() {
                    for field in [
                        &mut card.assignee,
                        &mut card.created_by,
                        &mut card.approved_by,
                    ] {
                        if field.as_ref() == Some(id) {
                            *field = None;
                        }
                    }
                }
                let removed = |actor: &mut Actor| {
                    if actor.agent() == Some(id) {
                        *actor = Actor::System;
                    }
                };
                inner
                    .comments
                    .iter_mut()
                    .for_each(|c| removed(&mut c.author));
                inner
                    .activity
                    .iter_mut()
                    .for_each(|a| removed(&mut a.actor));
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

impl BusRepository for InMemoryStore {
    async fn channel_by_slug(&self, team_id: &TeamId, slug: &str) -> RepoResult<Option<Channel>> {
        Ok(self
            .lock()
            .channels
            .iter()
            .find(|c| &c.team_id == team_id && c.slug == slug)
            .cloned())
    }

    async fn create_channel(&self, channel: &Channel) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.teams.contains_key(channel.team_id.as_str()) {
            return Err(RepoError::TeamNotFound(channel.team_id.clone()));
        }
        if inner
            .channels
            .iter()
            .any(|c| c.team_id == channel.team_id && c.slug == channel.slug)
        {
            return Err(RepoError::AlreadyExists(format!("#{}", channel.slug)));
        }
        inner.channels.push(channel.clone());
        Ok(())
    }

    async fn list_channels(&self, team_id: &TeamId) -> RepoResult<Vec<Channel>> {
        let mut list: Vec<_> = self
            .lock()
            .channels
            .iter()
            .filter(|c| &c.team_id == team_id)
            .cloned()
            .collect();
        list.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(list)
    }

    async fn insert_message(&self, message: &Message, deliveries: &[Delivery]) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.teams.contains_key(message.team_id.as_str()) {
            return Err(RepoError::TeamNotFound(message.team_id.clone()));
        }
        if inner.messages.contains_key(message.id.as_str()) {
            return Err(RepoError::AlreadyExists(message.id.to_string()));
        }
        if let Some(missing) = deliveries
            .iter()
            .find(|d| !inner.agents.contains_key(d.agent_id.as_str()))
        {
            return Err(RepoError::AgentNotFound(missing.agent_id.clone()));
        }
        inner
            .messages
            .insert(message.id.as_str().to_owned(), message.clone());
        for d in deliveries {
            inner.deliveries.insert(
                (
                    d.message_id.as_str().to_owned(),
                    d.agent_id.as_str().to_owned(),
                ),
                d.clone(),
            );
        }
        Ok(())
    }

    async fn get_message(&self, id: &MessageId) -> RepoResult<Option<Message>> {
        Ok(self.lock().messages.get(id.as_str()).cloned())
    }

    async fn deliveries_of(&self, message_id: &MessageId) -> RepoResult<Vec<Delivery>> {
        Ok(self
            .lock()
            .deliveries
            .values()
            .filter(|d| &d.message_id == message_id)
            .cloned()
            .collect())
    }

    async fn inbox(&self, agent_id: &AgentId, query: &InboxQuery) -> RepoResult<Vec<InboxItem>> {
        let inner = self.lock();
        let items = inner
            .messages
            .values()
            .filter(|m| {
                query
                    .after
                    .as_ref()
                    .is_none_or(|after| m.id.as_str() > after.as_str())
            })
            .filter_map(|m| {
                let key = (m.id.as_str().to_owned(), agent_id.as_str().to_owned());
                inner.deliveries.get(&key).map(|d| InboxItem {
                    message: m.clone(),
                    delivery: d.clone(),
                })
            })
            .filter(|item| !query.unread_only || item.delivery.state.is_unread())
            .take(query.limit as usize)
            .collect();
        Ok(items)
    }

    async fn mark_read(
        &self,
        agent_id: &AgentId,
        ids: &[MessageId],
        now: Millis,
    ) -> RepoResult<u32> {
        let mut inner = self.lock();
        let mut changed = 0;
        for id in ids {
            let key = (id.as_str().to_owned(), agent_id.as_str().to_owned());
            if let Some(d) = inner.deliveries.get_mut(&key) {
                if d.state.is_unread() {
                    d.state = DeliveryState::Read;
                    d.read_at = Some(now);
                    changed += 1;
                }
            }
        }
        Ok(changed)
    }

    async fn update_delivery(&self, delivery: &Delivery) -> RepoResult<()> {
        let mut inner = self.lock();
        let key = (
            delivery.message_id.as_str().to_owned(),
            delivery.agent_id.as_str().to_owned(),
        );
        match inner.deliveries.get_mut(&key) {
            Some(d) => {
                *d = delivery.clone();
                Ok(())
            }
            None => Err(RepoError::Corrupt(format!(
                "no delivery of {} to {}",
                delivery.message_id, delivery.agent_id
            ))),
        }
    }

    async fn timeline(
        &self,
        team_id: &TeamId,
        before: Option<&MessageId>,
        limit: u32,
    ) -> RepoResult<Vec<Message>> {
        Ok(self
            .lock()
            .messages
            .values()
            .rev()
            .filter(|m| &m.team_id == team_id)
            .filter(|m| before.is_none_or(|b| m.id.as_str() < b.as_str()))
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn replies_to(&self, id: &MessageId) -> RepoResult<Vec<Message>> {
        Ok(self
            .lock()
            .messages
            .values()
            .filter(|m| m.reply_to.as_ref() == Some(id))
            .cloned()
            .collect())
    }

    async fn unread_counts(&self, team_id: &TeamId) -> RepoResult<Vec<(AgentId, u32)>> {
        let inner = self.lock();
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for d in inner.deliveries.values().filter(|d| d.state.is_unread()) {
            let in_team = inner
                .messages
                .get(d.message_id.as_str())
                .is_some_and(|m| &m.team_id == team_id);
            if in_team {
                *counts.entry(d.agent_id.as_str().to_owned()).or_default() += 1;
            }
        }
        Ok(counts
            .into_iter()
            .map(|(id, n)| (AgentId::from_raw(id), n))
            .collect())
    }

    async fn prune_messages(&self, before: Millis) -> RepoResult<u64> {
        let mut inner = self.lock();
        let old: Vec<String> = inner
            .messages
            .values()
            .filter(|m| m.created_at < before)
            .map(|m| m.id.as_str().to_owned())
            .collect();
        for id in &old {
            inner.messages.remove(id);
        }
        inner.deliveries.retain(|(m, _), _| !old.contains(m));
        Ok(old.len() as u64)
    }
}

impl TokenRepository for InMemoryStore {
    async fn insert_token(&self, token: &str, record: &TokenRecord) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.agents.contains_key(record.agent_id.as_str()) {
            return Err(RepoError::AgentNotFound(record.agent_id.clone()));
        }
        if !inner.sessions.iter().any(|s| s.id == record.session_id) {
            return Err(RepoError::Corrupt(format!(
                "no session {}",
                record.session_id
            )));
        }
        if inner.tokens.contains_key(token) {
            return Err(RepoError::AlreadyExists("token".into()));
        }
        inner.tokens.insert(token.to_owned(), record.clone());
        Ok(())
    }

    async fn find_token(&self, token: &str, now: Millis) -> RepoResult<Option<TokenRecord>> {
        let inner = self.lock();
        let alive = |r: &TokenRecord| {
            r.expires_at > now
                && inner.agents.contains_key(r.agent_id.as_str())
                && inner.sessions.iter().any(|s| s.id == r.session_id)
        };
        Ok(inner.tokens.get(token).filter(|r| alive(r)).cloned())
    }

    async fn revoke_session_tokens(&self, session_id: &SessionId) -> RepoResult<u64> {
        let mut inner = self.lock();
        let before = inner.tokens.len();
        inner.tokens.retain(|_, r| &r.session_id != session_id);
        Ok((before - inner.tokens.len()) as u64)
    }

    async fn revoke_all_tokens(&self) -> RepoResult<u64> {
        let mut inner = self.lock();
        let n = inner.tokens.len() as u64;
        inner.tokens.clear();
        Ok(n)
    }
}

impl BoardRepository for InMemoryStore {
    async fn create_board(&self, board: &Board, columns: &[Column]) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.teams.contains_key(board.team_id.as_str()) {
            return Err(RepoError::TeamNotFound(board.team_id.clone()));
        }
        if inner.boards.contains_key(board.team_id.as_str()) {
            return Err(RepoError::AlreadyExists(format!(
                "board of {}",
                board.team_id
            )));
        }
        inner
            .boards
            .insert(board.team_id.as_str().to_owned(), board.clone());
        inner.columns.extend(columns.iter().cloned());
        Ok(())
    }

    async fn get_board(&self, team_id: &TeamId) -> RepoResult<Option<Board>> {
        Ok(self.lock().boards.get(team_id.as_str()).cloned())
    }

    async fn set_automations(
        &self,
        board_id: &BoardId,
        automations: &[Automation],
    ) -> RepoResult<()> {
        let mut inner = self.lock();
        let board = inner
            .boards
            .values_mut()
            .find(|b| &b.id == board_id)
            .ok_or_else(|| RepoError::Corrupt(format!("board {board_id} not found")))?;
        board.automations = automations.to_vec();
        Ok(())
    }

    async fn list_columns(&self, board_id: &BoardId) -> RepoResult<Vec<Column>> {
        let mut columns: Vec<Column> = self
            .lock()
            .columns
            .iter()
            .filter(|c| &c.board_id == board_id)
            .cloned()
            .collect();
        columns.sort_by_key(|c| c.position);
        Ok(columns)
    }

    async fn replace_columns(
        &self,
        board_id: &BoardId,
        columns: &[Column],
        moves: &[(ColumnId, ColumnId)],
        now: Millis,
    ) -> RepoResult<()> {
        let mut inner = self.lock();
        for (from, to) in moves {
            for card in inner.cards.values_mut().filter(|c| &c.column_id == from) {
                card.column_id = to.clone();
                card.column_since = now;
                card.version += 1;
                card.updated_at = now;
            }
        }
        let kept: Vec<&ColumnId> = columns.iter().map(|c| &c.id).collect();
        let orphan = inner.cards.values().any(|card| {
            inner.columns.iter().any(|c| {
                &c.board_id == board_id && c.id == card.column_id && !kept.contains(&&c.id)
            })
        });
        if orphan {
            return Err(RepoError::Corrupt(
                "a removed column still has cards".into(),
            ));
        }
        inner.columns.retain(|c| &c.board_id != board_id);
        inner.columns.extend(columns.iter().cloned());
        Ok(())
    }

    async fn insert_card(&self, card: &Card) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.teams.contains_key(card.team_id.as_str()) {
            return Err(RepoError::TeamNotFound(card.team_id.clone()));
        }
        if inner.cards.contains_key(card.id.as_str()) {
            return Err(RepoError::AlreadyExists(card.id.to_string()));
        }
        if !inner.columns.iter().any(|c| c.id == card.column_id) {
            return Err(RepoError::Corrupt(format!(
                "column {} not found",
                card.column_id
            )));
        }
        inner
            .cards
            .insert(card.id.as_str().to_owned(), card.clone());
        Ok(())
    }

    async fn get_card(&self, id: &CardId) -> RepoResult<Option<Card>> {
        Ok(self.lock().cards.get(id.as_str()).cloned())
    }

    async fn list_cards(&self, team_id: &TeamId, query: &CardQuery) -> RepoResult<Vec<Card>> {
        let inner = self.lock();
        let position = |id: &ColumnId| {
            inner
                .columns
                .iter()
                .find(|c| &c.id == id)
                .map_or(u32::MAX, |c| c.position)
        };
        let mut cards: Vec<Card> = inner
            .cards
            .values()
            .filter(|c| &c.team_id == team_id)
            .filter(|c| query.include_archived || c.archived_at.is_none())
            .filter(|c| {
                query
                    .column_id
                    .as_ref()
                    .is_none_or(|col| &c.column_id == col)
            })
            .filter(|c| query.assignee.is_none() || c.assignee == query.assignee)
            .filter(|c| !query.unassigned || c.assignee.is_none())
            .filter(|c| query.label.as_ref().is_none_or(|l| c.labels.contains(l)))
            .cloned()
            .collect();
        cards.sort_by(|a, b| {
            position(&a.column_id)
                .cmp(&position(&b.column_id))
                .then(a.position.cmp(&b.position))
                .then(a.id.as_str().cmp(b.id.as_str()))
        });
        Ok(cards)
    }

    async fn update_card(&self, card: &Card, guard: &WriteGuard) -> RepoResult<CardWrite> {
        let mut inner = self.lock();
        let Some(current) = inner.cards.get(card.id.as_str()) else {
            return Err(RepoError::Corrupt(format!("card {} not found", card.id)));
        };
        if current.version != guard.expected_version
            || (guard.require_unassigned && current.assignee.is_some())
        {
            return Ok(CardWrite::Stale);
        }
        if !inner.card_fits(card, guard) {
            return Ok(CardWrite::WipFull);
        }
        inner
            .cards
            .insert(card.id.as_str().to_owned(), card.clone());
        Ok(CardWrite::Written)
    }

    async fn dependencies(&self, team_id: &TeamId) -> RepoResult<Vec<(CardId, CardId)>> {
        let inner = self.lock();
        Ok(inner
            .dependencies
            .iter()
            .filter(|(t, _)| inner.cards.get(t).is_some_and(|c| &c.team_id == team_id))
            .map(|(t, d)| (CardId::from_raw(t.clone()), CardId::from_raw(d.clone())))
            .collect())
    }

    async fn add_dependency(&self, task: &CardId, depends_on: &CardId) -> RepoResult<()> {
        let mut inner = self.lock();
        if task == depends_on
            || !inner.cards.contains_key(task.as_str())
            || !inner.cards.contains_key(depends_on.as_str())
        {
            return Err(RepoError::Corrupt(format!(
                "dependency {task} -> {depends_on}"
            )));
        }
        inner
            .dependencies
            .insert((task.as_str().to_owned(), depends_on.as_str().to_owned()));
        Ok(())
    }

    async fn remove_dependency(&self, task: &CardId, depends_on: &CardId) -> RepoResult<()> {
        self.lock()
            .dependencies
            .remove(&(task.as_str().to_owned(), depends_on.as_str().to_owned()));
        Ok(())
    }

    async fn insert_comment(&self, comment: &Comment) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.cards.contains_key(comment.card_id.as_str()) {
            return Err(RepoError::Corrupt(format!(
                "card {} not found",
                comment.card_id
            )));
        }
        inner.comments.push(comment.clone());
        Ok(())
    }

    async fn comment_counts(&self, team_id: &TeamId) -> RepoResult<Vec<(CardId, u32)>> {
        let inner = self.lock();
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for c in &inner.comments {
            if inner
                .cards
                .get(c.card_id.as_str())
                .is_some_and(|card| &card.team_id == team_id)
            {
                *counts.entry(c.card_id.as_str().to_owned()).or_default() += 1;
            }
        }
        Ok(counts
            .into_iter()
            .map(|(id, n)| (CardId::from_raw(id), n))
            .collect())
    }

    async fn list_comments(&self, card_id: &CardId) -> RepoResult<Vec<Comment>> {
        let mut comments: Vec<Comment> = self
            .lock()
            .comments
            .iter()
            .filter(|c| &c.card_id == card_id)
            .cloned()
            .collect();
        comments.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then(a.id.as_str().cmp(b.id.as_str()))
        });
        Ok(comments)
    }

    async fn insert_activity(&self, activity: &Activity) -> RepoResult<()> {
        let mut inner = self.lock();
        if !inner.cards.contains_key(activity.card_id.as_str()) {
            return Err(RepoError::Corrupt(format!(
                "card {} not found",
                activity.card_id
            )));
        }
        inner.activity.push(activity.clone());
        Ok(())
    }

    async fn list_activity(&self, card_id: &CardId) -> RepoResult<Vec<Activity>> {
        let mut out: Vec<Activity> = self
            .lock()
            .activity
            .iter()
            .filter(|a| &a.card_id == card_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then(a.id.as_str().cmp(b.id.as_str()))
        });
        Ok(out)
    }

    async fn activity_since(&self, team_id: &TeamId, since: Millis) -> RepoResult<Vec<Activity>> {
        let inner = self.lock();
        let mut out: Vec<Activity> = inner
            .activity
            .iter()
            .filter(|a| a.created_at > since)
            .filter(|a| {
                inner
                    .cards
                    .get(a.card_id.as_str())
                    .is_some_and(|c| &c.team_id == team_id)
            })
            .cloned()
            .collect();
        out.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then(a.id.as_str().cmp(b.id.as_str()))
        });
        Ok(out)
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
