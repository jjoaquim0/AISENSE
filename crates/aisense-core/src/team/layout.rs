//! Estado da Sala da Equipe persistido em `teams.layout` (`docs/04`, `docs/09` T4).
//!
//! O conteúdo é da interface (modo de vista, ordem e posição dos painéis); o core só
//! garante que é um objeto JSON de tamanho razoável, para uma UI com defeito não
//! encher o banco nem gravar algo que não volta a abrir.

use serde_json::Value;

use crate::ids::TeamId;
use crate::repo::{RepoError, TeamRepository};

/// Um layout de 9 painéis livres ocupa ~1 KB; 64 KB é folga de sobra.
pub const LAYOUT_MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("the layout must be a JSON object")]
    NotAnObject,
    #[error("the layout is {0} bytes; the limit is {LAYOUT_MAX_BYTES}")]
    TooLarge(usize),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl LayoutError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotAnObject => "invalid_layout",
            Self::TooLarge(_) => "layout_too_large",
            Self::Repo(e) => e.code(),
        }
    }
}

/// Grava o layout sem mexer em `updated_at`: arrastar um painel não é "atividade" da
/// equipe e não deve reordenar a lista de equipes.
pub async fn save_layout<S: TeamRepository>(
    store: &S,
    team_id: &TeamId,
    layout: Value,
) -> Result<(), LayoutError> {
    if !layout.is_object() {
        return Err(LayoutError::NotAnObject);
    }
    let size = layout.to_string().len();
    if size > LAYOUT_MAX_BYTES {
        return Err(LayoutError::TooLarge(size));
    }
    let mut team = store
        .get_team(team_id)
        .await?
        .ok_or_else(|| RepoError::TeamNotFound(team_id.clone()))?;
    team.layout = layout;
    store.update_team(&team).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use serde_json::json;

    use super::*;
    use crate::repo::InMemoryStore;
    use crate::team::{Team, TeamDraft};

    async fn team(store: &InMemoryStore) -> Team {
        let t = Team::create(
            &TeamDraft {
                name: "Squad".into(),
                workdir: "/tmp".into(),
                ..TeamDraft::default()
            },
            7,
        )
        .unwrap();
        store.create_team(&t).await.unwrap();
        t
    }

    #[tokio::test]
    async fn saves_and_keeps_updated_at() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        let layout = json!({ "grid": { "preset": "4", "order": ["agt_a", "agt_b"] } });
        save_layout(&store, &t.id, layout.clone()).await.unwrap();
        let saved = store.get_team(&t.id).await.unwrap().unwrap();
        assert_eq!(saved.layout, layout);
        assert_eq!(saved.updated_at, 7);
    }

    #[tokio::test]
    async fn refuses_non_objects_and_huge_layouts() {
        let store = InMemoryStore::new();
        let t = team(&store).await;
        assert!(matches!(
            save_layout(&store, &t.id, json!([1, 2])).await,
            Err(LayoutError::NotAnObject)
        ));
        let huge = json!({ "x": "a".repeat(LAYOUT_MAX_BYTES) });
        assert!(matches!(
            save_layout(&store, &t.id, huge).await,
            Err(LayoutError::TooLarge(_))
        ));
        assert!(matches!(
            save_layout(&store, &TeamId::new(), json!({})).await,
            Err(LayoutError::Repo(RepoError::TeamNotFound(_)))
        ));
    }
}
