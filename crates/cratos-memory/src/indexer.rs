//! Turn indexer — incremental indexing of new turns.
//!
//! After each orchestrator execution, the indexer:
//! 1. Decomposes messages into turns (skipping already-indexed ones)
//! 2. Extracts entities from each new turn
//! 3. Persists turns, entities, and edges to GraphStore
//! 4. Embeds turn summaries and adds them to the VectorIndex

use crate::decomposer;
use crate::extractor;
use crate::store::GraphStore;
use crate::types::{Entity, EntityKind, EntityRelation, TurnEntityEdge};
use chrono::Utc;
use cratos_llm::Message;
use tracing::{debug, warn};
use uuid::Uuid;

/// Indexes conversation turns into the graph store and vector index.
pub struct TurnIndexer {
    store: GraphStore,
    /// Optional: embed callback. If None, vector indexing is skipped.
    embedder: Option<Box<dyn EmbedAndStore>>,
}

/// Trait for embedding a text and storing the vector.
///
/// Decouples the indexer from concrete EmbeddingProvider + VectorIndex types.
#[async_trait::async_trait]
pub trait EmbedAndStore: Send + Sync {
    /// Embed `text` and store the resulting vector under `id`.
    async fn embed_and_store(&self, id: &str, text: &str) -> crate::Result<()>;
}

impl TurnIndexer {
    /// Create an indexer without vector embedding (graph-only).
    pub fn new(store: GraphStore) -> Self {
        Self {
            store,
            embedder: None,
        }
    }

    /// Create an indexer with vector embedding support.
    pub fn with_embedder(store: GraphStore, embedder: Box<dyn EmbedAndStore>) -> Self {
        Self {
            store,
            embedder: Some(embedder),
        }
    }

    /// Index new turns from a completed session.
    ///
    /// Only turns with `turn_index` greater than the previously indexed max
    /// are processed (incremental).
    pub async fn index_session(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> crate::Result<u32> {
        let existing_max = self.store.max_turn_index(session_id).await?;
        let turns = decomposer::decompose(session_id, messages, existing_max);

        if turns.is_empty() {
            debug!(session_id, "No new turns to index");
            return Ok(0);
        }

        let mut indexed = 0u32;
        for turn in &turns {
            // 1. Insert turn
            self.store.insert_turn(turn).await?;

            // 2. Extract entities and relations
            let extracted = extractor::extract(&turn.content);

            // 3. Persist entities + edges
            let mut entity_ids = Vec::with_capacity(extracted.entities.len());
            for ext in &extracted.entities {
                let entity_id = self.resolve_entity(&ext.name, ext.kind).await?;
                entity_ids.push(entity_id.clone());

                let edge = TurnEntityEdge {
                    turn_id: turn.id.clone(),
                    entity_id,
                    relevance: ext.relevance,
                };
                self.store.insert_edge(&edge).await?;
            }

            // 4. Persist relations
            for rel_ext in &extracted.relations {
                if let (Some(from), Some(to)) = (
                    self.store.get_entity_by_name(&rel_ext.from_entity).await?,
                    self.store.get_entity_by_name(&rel_ext.to_entity).await?,
                ) {
                    let rel = EntityRelation {
                        from_entity_id: from.id,
                        to_entity_id: to.id,
                        kind: rel_ext.kind,
                    };
                    self.store.insert_relation(&rel).await?;
                }
            }

            // 5. Update co-occurrence
            if entity_ids.len() > 1 {
                self.store.update_cooccurrence(&entity_ids).await?;
            }

            // 6. Embed summary → vector index
            if let Some(embedder) = &self.embedder {
                if let Err(e) = embedder.embed_and_store(&turn.id, &turn.summary).await {
                    warn!(turn_id = %turn.id, error = %e, "Embedding failed, skipping");
                }
            }

            indexed += 1;
        }

        debug!(session_id, indexed, "Indexed new turns");
        Ok(indexed)
    }

    /// Look up or create an entity by name, returning its ID.
    async fn resolve_entity(&self, name: &str, kind: EntityKind) -> crate::Result<String> {
        if let Some(existing) = self.store.get_entity_by_name(name).await? {
            // Upsert increments mention_count
            self.store.upsert_entity(&existing).await?;
            return Ok(existing.id);
        }
        let entity = Entity {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            kind,
            first_seen: Utc::now(),
            mention_count: 1,
        };
        self.store.upsert_entity(&entity).await?;
        Ok(entity.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::GraphStore;

    #[tokio::test]
    async fn test_index_session_basic() {
        let store = GraphStore::in_memory().await.unwrap();
        let indexer = TurnIndexer::new(store.clone());

        let messages = vec![
            Message::system("You are a helper."),
            Message::user("Fix orchestrator.rs in cratos-core"),
            Message::assistant("I'll fix it now."),
            Message::user("Also check store.rs"),
        ];

        let count = indexer.index_session("s1", &messages).await.unwrap();
        assert_eq!(count, 3); // system skipped

        // Verify graph
        assert_eq!(store.turn_count().await.unwrap(), 3);
        assert!(store.entity_count().await.unwrap() >= 2); // at least orchestrator.rs, cratos-core

        // Verify entities linked to first user turn
        let turns = store.get_turns_by_session("s1").await.unwrap();
        let first_turn = &turns[0];
        let entities = store.get_entities_for_turn(&first_turn.id).await.unwrap();
        assert!(entities.iter().any(|e| e.name == "orchestrator.rs"));
    }

    #[tokio::test]
    async fn test_incremental_indexing() {
        let store = GraphStore::in_memory().await.unwrap();
        let indexer = TurnIndexer::new(store.clone());

        let messages1 = vec![Message::user("Hello"), Message::assistant("Hi")];
        let count1 = indexer.index_session("s1", &messages1).await.unwrap();
        assert_eq!(count1, 2);

        // Add more messages and re-index — only new turns should be indexed
        let messages2 = vec![
            Message::user("Hello"),
            Message::assistant("Hi"),
            Message::user("What's new?"),
        ];
        let count2 = indexer.index_session("s1", &messages2).await.unwrap();
        assert_eq!(count2, 1); // only the new user message

        assert_eq!(store.turn_count().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_entity_mention_count() {
        let store = GraphStore::in_memory().await.unwrap();
        let indexer = TurnIndexer::new(store.clone());

        let messages = vec![
            Message::user("Look at orchestrator.rs"),
            Message::assistant("Checking orchestrator.rs now"),
        ];
        indexer.index_session("s1", &messages).await.unwrap();

        let entity = store
            .get_entity_by_name("orchestrator.rs")
            .await
            .unwrap()
            .unwrap();
        assert!(entity.mention_count >= 2);
    }
}
