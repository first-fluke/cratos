//! Cratos Memory — Graph RAG Conversation Memory
//!
//! Decomposes conversations into turns, extracts entities, builds an
//! entity-turn-entity graph, and retrieves relevant past turns via
//! hybrid embedding + graph traversal scoring.
//!
//! # Architecture
//!
//! ```text
//! Messages ──► TurnDecomposer ──► Turns
//!                                   │
//!                          EntityExtractor
//!                                   │
//!                              GraphStore (SQLite)
//!                             ╱           ╲
//!                    VectorIndex      BFS traversal
//!                             ╲           ╱
//!                          HybridScorer
//!                                   │
//!                          RetrievedTurns
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "embeddings")]
pub mod bridge;
pub mod decomposer;
pub mod error;
pub mod extractor;
pub mod indexer;
pub mod retriever;
pub mod scorer;
pub mod store;
pub mod types;

pub use error::{Error, Result};
pub use indexer::{EmbedAndStore, TurnIndexer};
pub use retriever::{GraphRagRetriever, VectorSearch};
pub use scorer::ScoringWeights;
pub use store::GraphStore;
pub use types::{
    Entity, EntityKind, ExplicitMemory, ExtractedEntity, RetrievedTurn, Turn, TurnEntityEdge,
    TurnRole,
};

#[cfg(feature = "embeddings")]
pub use bridge::VectorBridge;

use chrono::Utc;
use cratos_llm::Message;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// High-level facade combining indexing and retrieval.
///
/// Provides a simple API for the orchestrator to index conversations
/// and retrieve relevant past turns.
///
/// # Modes
///
/// - **Graph-only** (`from_path` / `in_memory`): entity graph search, no embeddings.
/// - **With embeddings** (`with_vector_bridge`): hybrid search combining embedding
///   similarity + entity graph traversal for better recall.
pub struct GraphMemory {
    store: GraphStore,
    /// When set, indexing also embeds turn summaries and retrieval uses vector seeds.
    vector_bridge: Option<Arc<dyn EmbedAndStore + Send + Sync>>,
    /// Search bridge (same object as above, different trait).
    vector_search: Option<Arc<dyn VectorSearch>>,
    /// Separate vector bridge for explicit memories (different HNSW index).
    explicit_embed: Option<Arc<dyn EmbedAndStore + Send + Sync>>,
    /// Search bridge for explicit memories.
    explicit_search: Option<Arc<dyn VectorSearch>>,
}

impl GraphMemory {
    /// Create a new `GraphMemory` backed by an SQLite file (graph-only).
    pub async fn from_path(path: &Path) -> Result<Self> {
        let store = GraphStore::from_path(path).await?;
        Ok(Self {
            store,
            vector_bridge: None,
            vector_search: None,
            explicit_embed: None,
            explicit_search: None,
        })
    }

    /// Create an in-memory `GraphMemory` (useful for tests).
    pub async fn in_memory() -> Result<Self> {
        let store = GraphStore::in_memory().await?;
        Ok(Self {
            store,
            vector_bridge: None,
            vector_search: None,
            explicit_embed: None,
            explicit_search: None,
        })
    }

    /// Attach a vector bridge for embedding-based hybrid search.
    ///
    /// The bridge must implement both [`EmbedAndStore`] (for indexing) and
    /// [`VectorSearch`] (for retrieval). [`VectorBridge`] satisfies both.
    #[cfg(feature = "embeddings")]
    pub fn with_vector_bridge(mut self, bridge: Arc<VectorBridge>) -> Self {
        self.vector_bridge = Some(bridge.clone() as Arc<dyn EmbedAndStore + Send + Sync>);
        self.vector_search = Some(bridge as Arc<dyn VectorSearch>);
        self
    }

    /// Attach a separate vector bridge for explicit memory embeddings.
    #[cfg(feature = "embeddings")]
    pub fn with_explicit_vector_bridge(mut self, bridge: Arc<VectorBridge>) -> Self {
        self.explicit_embed = Some(bridge.clone() as Arc<dyn EmbedAndStore + Send + Sync>);
        self.explicit_search = Some(bridge as Arc<dyn VectorSearch>);
        self
    }

    /// Index new turns from a completed session.
    ///
    /// If a vector bridge is attached, turn summaries are also embedded.
    /// Returns the number of newly indexed turns.
    pub async fn index_session(&self, session_id: &str, messages: &[Message]) -> Result<u32> {
        let indexer = if let Some(bridge) = &self.vector_bridge {
            TurnIndexer::with_embedder(
                self.store.clone(),
                Box::new(BridgeAdapter(Arc::clone(bridge))),
            )
        } else {
            TurnIndexer::new(self.store.clone())
        };
        let count = indexer.index_session(session_id, messages).await?;
        debug!(session_id, count, "GraphMemory indexed session");
        Ok(count)
    }

    /// Retrieve relevant past turns for a query.
    ///
    /// Uses hybrid search (embedding + graph) if a vector bridge is attached,
    /// otherwise falls back to entity-graph-only retrieval.
    pub async fn retrieve(
        &self,
        query: &str,
        max_turns: usize,
        max_tokens: u32,
    ) -> Result<Vec<RetrievedTurn>> {
        let retriever = if let Some(vs) = &self.vector_search {
            GraphRagRetriever::with_vector_search(
                self.store.clone(),
                Box::new(SearchAdapter(Arc::clone(vs))),
            )
        } else {
            GraphRagRetriever::new(self.store.clone())
        };
        retriever.retrieve(query, max_turns, max_tokens).await
    }

    /// Convert retrieved turns into LLM messages for context injection.
    pub fn turns_to_messages(turns: &[RetrievedTurn]) -> Vec<Message> {
        let mut messages = Vec::with_capacity(turns.len());
        for rt in turns {
            let msg = match rt.turn.role {
                TurnRole::User => Message::user(&rt.turn.content),
                TurnRole::Assistant => Message::assistant(&rt.turn.content),
            };
            messages.push(msg);
        }
        messages
    }

    /// Get the number of indexed turns.
    pub async fn turn_count(&self) -> Result<u32> {
        self.store.turn_count().await
    }

    /// Get the number of known entities.
    pub async fn entity_count(&self) -> Result<u32> {
        self.store.entity_count().await
    }

    // ── Graph Data Export API ────────────────────────────────────

    /// List all entities (for graph visualization).
    pub async fn list_entities(&self, limit: u32) -> Result<Vec<Entity>> {
        self.store.list_all_entities(limit).await
    }

    /// List all co-occurrence edges (for graph visualization).
    /// Returns tuples of (entity_id_a, entity_id_b, cooccurrence_count).
    pub async fn list_cooccurrences(&self, limit: u32) -> Result<Vec<(String, String, u32)>> {
        self.store.list_all_cooccurrences(limit).await
    }

    // ── Explicit Memory API ──────────────────────────────────────

    /// Save an explicit memory with entity extraction and optional embedding.
    ///
    /// Returns the memory ID (new UUID or existing if name already exists).
    pub async fn save_memory(
        &self,
        name: &str,
        content: &str,
        category: &str,
        tags: &[String],
    ) -> Result<String> {
        let now = Utc::now();
        let mem = ExplicitMemory {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            content: content.to_string(),
            category: category.to_string(),
            tags: tags.to_vec(),
            created_at: now,
            updated_at: now,
            access_count: 0,
        };

        // 1. Persist to SQLite
        self.store.save_explicit_memory(&mem).await?;

        // Re-fetch to get the canonical ID (upsert may keep old ID)
        let saved = self
            .store
            .get_explicit_by_name(name)
            .await?
            .ok_or_else(|| Error::Internal("Memory not found after save".into()))?;
        let mem_id = saved.id.clone();

        // 2. Extract entities and link them
        let entities = extractor::extract(content);
        for ext in &entities {
            let entity = types::Entity {
                id: Uuid::new_v4().to_string(),
                name: ext.name.clone(),
                kind: ext.kind,
                first_seen: now,
                mention_count: 1,
            };
            self.store.upsert_entity(&entity).await?;
            // Resolve the canonical entity (upsert may have kept old ID)
            if let Some(resolved) = self.store.get_entity_by_name(&ext.name).await? {
                self.store
                    .insert_memory_entity_edge(&mem_id, &resolved.id, ext.relevance)
                    .await?;
            }
        }

        // 3. Embed content (if explicit vector bridge is available)
        if let Some(embedder) = &self.explicit_embed {
            if let Err(e) = embedder.embed_and_store(&mem_id, content).await {
                warn!(error = %e, "Failed to embed explicit memory");
            }
        }

        debug!(name, mem_id = %mem_id, entities = entities.len(), "Explicit memory saved");
        Ok(mem_id)
    }

    /// Hybrid recall of explicit memories.
    ///
    /// Combines: exact name match, vector search, entity graph, LIKE search.
    pub async fn recall_memories(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<ExplicitMemory>> {
        let mut scored: HashMap<String, (ExplicitMemory, f32)> = HashMap::new();

        // 1. Exact name match (highest score)
        if let Some(mem) = self.store.get_explicit_by_name(query).await? {
            scored.insert(mem.id.clone(), (mem, 10.0));
        }

        // 2. Vector search
        if let Some(vs) = &self.explicit_search {
            match vs.search(query, max_results * 2).await {
                Ok(results) => {
                    for (mem_id, sim) in results {
                        if let Some(mem) = self.store.get_explicit_by_id(&mem_id).await? {
                            scored
                                .entry(mem.id.clone())
                                .and_modify(|(_, s)| *s += sim)
                                .or_insert((mem, sim));
                        }
                    }
                }
                Err(e) => warn!(error = %e, "Explicit memory vector search failed"),
            }
        }

        // 3. Entity graph: extract entities from query, find linked memories
        let query_entities = extractor::extract(query);
        for ext in &query_entities {
            if let Some(entity) = self.store.get_entity_by_name(&ext.name).await? {
                match self.store.get_explicit_by_entity(&entity.id).await {
                    Ok(mems) => {
                        for mem in mems {
                            scored
                                .entry(mem.id.clone())
                                .and_modify(|(_, s)| *s += 0.5)
                                .or_insert((mem, 0.5));
                        }
                    }
                    Err(e) => warn!(error = %e, "Entity-based memory lookup failed"),
                }
            }
        }

        // 4. LIKE search (tokenized — each word searched independently)
        // Split query into meaningful tokens so "sns 자동화 성공한 방법" → ["sns", "자동화", "성공한", "방법"]
        let tokens: Vec<&str> = query
            .split_whitespace()
            .filter(|w| w.chars().count() >= 2)
            .collect();
        let search_terms: Vec<&str> = if tokens.is_empty() {
            vec![query]
        } else {
            tokens
        };
        for term in &search_terms {
            match self
                .store
                .search_explicit(term, None, max_results as u32)
                .await
            {
                Ok(mems) => {
                    for mem in mems {
                        scored
                            .entry(mem.id.clone())
                            .and_modify(|(_, s)| *s += 0.3)
                            .or_insert((mem, 0.3));
                    }
                }
                Err(e) => warn!(error = %e, term, "LIKE-based memory search failed"),
            }
        }

        // 5. Sort by score, take top results
        let mut results: Vec<(ExplicitMemory, f32)> = scored.into_values().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(max_results);

        // 6. Increment access counts
        for (mem, _) in &results {
            let _ = self.store.increment_access_count(&mem.id).await;
        }

        debug!(
            query,
            results = results.len(),
            "Explicit memory recall complete"
        );
        Ok(results.into_iter().map(|(m, _)| m).collect())
    }

    /// List explicit memories, optionally filtered by category.
    pub async fn list_memories(
        &self,
        category: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ExplicitMemory>> {
        self.store.list_explicit(category, limit).await
    }

    /// Delete an explicit memory by name.
    pub async fn delete_memory(&self, name: &str) -> Result<bool> {
        self.store.delete_explicit(name).await
    }

    /// Update an explicit memory (partial update).
    pub async fn update_memory(
        &self,
        name: &str,
        content: Option<&str>,
        category: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<()> {
        let existing = self
            .store
            .get_explicit_by_name(name)
            .await?
            .ok_or_else(|| Error::Internal(format!("Memory '{name}' not found")))?;

        let updated = ExplicitMemory {
            id: existing.id.clone(),
            name: existing.name.clone(),
            content: content.unwrap_or(&existing.content).to_string(),
            category: category.unwrap_or(&existing.category).to_string(),
            tags: tags.map(|t| t.to_vec()).unwrap_or(existing.tags),
            created_at: existing.created_at,
            updated_at: Utc::now(),
            access_count: existing.access_count,
        };

        self.store.save_explicit_memory(&updated).await?;

        // Re-embed if content changed
        if content.is_some() {
            if let Some(embedder) = &self.explicit_embed {
                if let Err(e) = embedder
                    .embed_and_store(&updated.id, &updated.content)
                    .await
                {
                    warn!(error = %e, "Failed to re-embed updated memory");
                }
            }
        }

        debug!(name, "Explicit memory updated");
        Ok(())
    }

    /// Re-embed all explicit memories that exist in DB but not in vector index.
    /// Call this once during server startup to backfill.
    pub async fn reindex_explicit_memories(&self) -> Result<usize> {
        let embedder = match &self.explicit_embed {
            Some(e) => e,
            None => {
                info!("No explicit embedding bridge, skipping reindex");
                return Ok(0);
            }
        };
        let all = self.store.list_explicit(None, 1000).await?;
        info!(total = all.len(), "Reindexing explicit memories");
        let mut count = 0;
        for mem in &all {
            match embedder.embed_and_store(&mem.id, &mem.content).await {
                Ok(()) => {
                    count += 1;
                    debug!(name = %mem.name, "Re-embedded explicit memory");
                }
                Err(e) => {
                    // AlreadyExists is fine — means it was already indexed
                    let msg = e.to_string();
                    if !msg.contains("already") {
                        warn!(name = %mem.name, error = %e, "Failed to re-embed explicit memory");
                    }
                }
            }
        }
        if count > 0 {
            info!(
                count,
                total = all.len(),
                "Re-indexed explicit memories with embeddings"
            );
        }
        Ok(count)
    }
}

// ── Internal adapters: Arc<dyn Trait> → Box<dyn Trait> ──────────────

/// Wraps `Arc<dyn EmbedAndStore>` so it can be passed as `Box<dyn EmbedAndStore>`.
struct BridgeAdapter(Arc<dyn EmbedAndStore + Send + Sync>);

#[async_trait::async_trait]
impl EmbedAndStore for BridgeAdapter {
    async fn embed_and_store(&self, id: &str, text: &str) -> Result<()> {
        self.0.embed_and_store(id, text).await
    }
}

/// Wraps `Arc<dyn VectorSearch>` so it can be passed as `Box<dyn VectorSearch>`.
struct SearchAdapter(Arc<dyn VectorSearch>);

#[async_trait::async_trait]
impl VectorSearch for SearchAdapter {
    async fn search(&self, query: &str, top_k: usize) -> Result<Vec<(String, f32)>> {
        self.0.search(query, top_k).await
    }
}
