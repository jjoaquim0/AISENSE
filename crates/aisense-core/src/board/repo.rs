//! Porta de persistência do quadro. SQLite em `aisense-store`, memória em
//! `repo::InMemoryStore` — mesmo contrato, testado contra os dois (F06-02).

use std::future::Future;

use super::automation::Automation;
use super::model::{Activity, Board, Card, Column, Comment};
use crate::ids::{AgentId, BoardId, CardId, ColumnId, TeamId};
use crate::repo::RepoResult;
use crate::time::Millis;

/// Filtro da listagem de cartões. Tudo `None` = todos os cartões vivos da equipe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardQuery {
    pub column_id: Option<ColumnId>,
    pub assignee: Option<AgentId>,
    /// Só sem responsável.
    pub unassigned: bool,
    pub label: Option<String>,
    pub include_archived: bool,
}

/// Condições de uma gravação de cartão, conferidas **na mesma instrução** que grava — é o
/// que torna `claim` atômico e o WIP aplicado de verdade sob concorrência (F06-03).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteGuard {
    /// Versão lida; a gravação só vale se o banco ainda estiver nela.
    pub expected_version: i64,
    /// `claim`: só grava se ninguém pegou.
    pub require_unassigned: bool,
    /// Limites da coluna de destino (a do cartão gravado), contando os outros cartões.
    pub wip_limit: Option<u32>,
    pub wip_per_agent: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardWrite {
    Written,
    /// A versão mudou ou (com `require_unassigned`) alguém pegou antes.
    Stale,
    /// A coluna de destino está cheia.
    WipFull,
}

pub trait BoardRepository: Send + Sync {
    /// O quadro e suas colunas numa transação. `AlreadyExists` se a equipe já tem quadro.
    fn create_board(
        &self,
        board: &Board,
        columns: &[Column],
    ) -> impl Future<Output = RepoResult<()>> + Send;
    fn get_board(&self, team_id: &TeamId)
        -> impl Future<Output = RepoResult<Option<Board>>> + Send;
    fn set_automations(
        &self,
        board_id: &BoardId,
        automations: &[Automation],
    ) -> impl Future<Output = RepoResult<()>> + Send;
    /// Por `position`.
    fn list_columns(
        &self,
        board_id: &BoardId,
    ) -> impl Future<Output = RepoResult<Vec<Column>>> + Send;
    /// Troca as colunas do quadro de uma vez: `columns` é a lista final (novas, renomeadas,
    /// reordenadas); as que saírem são removidas depois de mover os cartões conforme
    /// `moves` (de → para). Tudo numa transação.
    fn replace_columns(
        &self,
        board_id: &BoardId,
        columns: &[Column],
        moves: &[(ColumnId, ColumnId)],
        now: Millis,
    ) -> impl Future<Output = RepoResult<()>> + Send;

    fn insert_card(&self, card: &Card) -> impl Future<Output = RepoResult<()>> + Send;
    fn get_card(&self, id: &CardId) -> impl Future<Output = RepoResult<Option<Card>>> + Send;
    /// Por coluna, depois `position`, depois criação.
    fn list_cards(
        &self,
        team_id: &TeamId,
        query: &CardQuery,
    ) -> impl Future<Output = RepoResult<Vec<Card>>> + Send;
    /// Grava todos os campos mutáveis de `card` se `guard` for cumprido.
    fn update_card(
        &self,
        card: &Card,
        guard: &WriteGuard,
    ) -> impl Future<Output = RepoResult<CardWrite>> + Send;

    /// `(cartão, depende de)` de toda a equipe.
    fn dependencies(
        &self,
        team_id: &TeamId,
    ) -> impl Future<Output = RepoResult<Vec<(CardId, CardId)>>> + Send;
    fn add_dependency(
        &self,
        task: &CardId,
        depends_on: &CardId,
    ) -> impl Future<Output = RepoResult<()>> + Send;
    fn remove_dependency(
        &self,
        task: &CardId,
        depends_on: &CardId,
    ) -> impl Future<Output = RepoResult<()>> + Send;

    fn insert_comment(&self, comment: &Comment) -> impl Future<Output = RepoResult<()>> + Send;
    /// Quantos comentários cada cartão da equipe tem (os sem comentário ficam de fora).
    fn comment_counts(
        &self,
        team_id: &TeamId,
    ) -> impl Future<Output = RepoResult<Vec<(CardId, u32)>>> + Send;
    /// Mais antigo primeiro.
    fn list_comments(
        &self,
        card_id: &CardId,
    ) -> impl Future<Output = RepoResult<Vec<Comment>>> + Send;

    /// Só insere: o histórico é imutável.
    fn insert_activity(&self, activity: &Activity) -> impl Future<Output = RepoResult<()>> + Send;
    /// Mais antiga primeiro.
    fn list_activity(
        &self,
        card_id: &CardId,
    ) -> impl Future<Output = RepoResult<Vec<Activity>>> + Send;
    /// O que mudou no quadro desde um instante ("3 cartões mudaram desde que você saiu").
    fn activity_since(
        &self,
        team_id: &TeamId,
        since: Millis,
    ) -> impl Future<Output = RepoResult<Vec<Activity>>> + Send;
}
