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
    Entity, EntityKind, ExtractedEntity, RetrievedTurn, Turn, TurnEntityEdge, TurnRole,
};

#[cfg(feature = "embeddings")]
pub use bridge::VectorBridge;

use cratos_llm::Message;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, warn};

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
}

impl GraphMemory {
    /// Create a new `GraphMemory` backed by an SQLite file (graph-only).
    pub async fn from_path(path: &Path) -> Result<Self> {
        let store = GraphStore::from_path(path).await?;
        Ok(Self {
            store,
            vector_bridge: None,
            vector_search: None,
        })
    }

    /// Create an in-memory `GraphMemory` (useful for tests).
    pub async fn in_memory() -> Result<Self> {
        let store = GraphStore::in_memory().await?;
        Ok(Self {
            store,
            vector_bridge: None,
            vector_search: None,
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

    /// Index new turns from a completed session.
    ///
    /// If a vector bridge is attached, turn summaries are also embedded.
    /// Returns the number of newly indexed turns.
    pub async fn index_session(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> Result<u32> {
        let indexer = if let Some(bridge) = &self.vector_bridge {
            TurnIndexer::with_embedder(self.store.clone(), Box::new(BridgeAdapter(Arc::clone(bridge))))
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
