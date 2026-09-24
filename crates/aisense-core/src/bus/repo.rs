//! Porta de persistência do barramento. SQLite em `aisense-store`, memória em
//! `repo::InMemoryStore` — mesmo contrato, testado contra os dois (F05-02).

use std::future::Future;

use super::model::{Channel, Delivery, InboxItem, Message};
use crate::ids::{AgentId, ChannelId, MessageId, TeamId};
use crate::repo::RepoResult;
use crate::time::Millis;

/// Filtro da caixa de entrada.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InboxQuery {
    /// Só `pending`/`delivered`.
    pub unread_only: bool,
    /// Só mensagens depois desta (cursor).
    pub after: Option<MessageId>,
    pub limit: u32,
}

pub trait BusRepository: Send + Sync {
    fn channel_by_slug(
        &self,
        team_id: &TeamId,
        slug: &str,
    ) -> impl Future<Output = RepoResult<Option<Channel>>> + Send;
    /// `AlreadyExists` com slug repetido na equipe.
    fn create_channel(&self, channel: &Channel) -> impl Future<Output = RepoResult<()>> + Send;
    fn list_channels(
        &self,
        team_id: &TeamId,
    ) -> impl Future<Output = RepoResult<Vec<Channel>>> + Send;
    fn set_channel_topic(
        &self,
        id: &ChannelId,
        topic: &str,
    ) -> impl Future<Output = RepoResult<()>> + Send;
    /// Apaga o canal e, em cascata, as mensagens dele e as inscrições.
    fn delete_channel(&self, id: &ChannelId) -> impl Future<Output = RepoResult<()>> + Send;
    /// Inscritos, na ordem de `agent_id`. Vazio = canal aberto (vai para a equipe toda).
    fn channel_members(
        &self,
        id: &ChannelId,
    ) -> impl Future<Output = RepoResult<Vec<AgentId>>> + Send;
    /// Troca a lista inteira. Agente de outra equipe ou inexistente: `AgentNotFound`.
    fn set_channel_members(
        &self,
        id: &ChannelId,
        members: &[AgentId],
    ) -> impl Future<Output = RepoResult<()>> + Send;

    /// A mensagem e uma entrega por destinatário, na mesma transação (invariante I3).
    fn insert_message(
        &self,
        message: &Message,
        deliveries: &[Delivery],
    ) -> impl Future<Output = RepoResult<()>> + Send;
    fn get_message(
        &self,
        id: &MessageId,
    ) -> impl Future<Output = RepoResult<Option<Message>>> + Send;
    /// As entregas de uma mensagem (recibos na linha do tempo).
    fn deliveries_of(
        &self,
        message_id: &MessageId,
    ) -> impl Future<Output = RepoResult<Vec<Delivery>>> + Send;
    /// Mais antiga primeiro — a ordem em que o agente deve ler.
    fn inbox(
        &self,
        agent_id: &AgentId,
        query: &InboxQuery,
    ) -> impl Future<Output = RepoResult<Vec<InboxItem>>> + Send;
    /// Marca como lidas as mensagens dadas deste agente; devolve quantas mudaram.
    fn mark_read(
        &self,
        agent_id: &AgentId,
        ids: &[MessageId],
        now: Millis,
    ) -> impl Future<Output = RepoResult<u32>> + Send;
    /// Troca estado, tentativas e erro de uma entrega existente.
    fn update_delivery(&self, delivery: &Delivery) -> impl Future<Output = RepoResult<()>> + Send;
    /// Mais recente primeiro, antes do cursor.
    fn timeline(
        &self,
        team_id: &TeamId,
        before: Option<&MessageId>,
        limit: u32,
    ) -> impl Future<Output = RepoResult<Vec<Message>>> + Send;
    /// Respostas (`reply_to = id`), mais antiga primeiro.
    fn replies_to(&self, id: &MessageId) -> impl Future<Output = RepoResult<Vec<Message>>> + Send;
    /// Não lidas por agente da equipe (badge da sidebar).
    fn unread_counts(
        &self,
        team_id: &TeamId,
    ) -> impl Future<Output = RepoResult<Vec<(AgentId, u32)>>> + Send;
    /// Retenção: apaga mensagens anteriores a `before` (e as entregas, em cascata).
    fn prune_messages(&self, before: Millis) -> impl Future<Output = RepoResult<u64>> + Send;
}
