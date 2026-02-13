//! Bridge — concrete implementations of [`EmbedAndStore`] and [`VectorSearch`]
//! backed by `cratos-llm::EmbeddingProvider` + `cratos-search::VectorIndex`.
//!
//! Enabled only when the `embeddings` feature is active.

use crate::indexer::EmbedAndStore;
use crate::retriever::VectorSearch;
use cratos_llm::embeddings::SharedEmbeddingProvider;
use cratos_search::VectorIndex;
use std::sync::Arc;
use tracing::debug;

/// Bridges `EmbeddingProvider` + `VectorIndex` into the Graph RAG traits.
pub struct VectorBridge {
    embedder: SharedEmbeddingProvider,
    index: Arc<VectorIndex>,
}

impl VectorBridge {
    /// Create a new bridge.
    pub fn new(embedder: SharedEmbeddingProvider, index: Arc<VectorIndex>) -> Self {
        Self { embedder, index }
    }
}

#[async_trait::async_trait]
impl EmbedAndStore for VectorBridge {
    async fn embed_and_store(&self, id: &str, text: &str) -> crate::Result<()> {
        let vector = self
            .embedder
            .embed(text)
            .await
            .map_err(|e| crate::Error::Embedding(e.to_string()))?;

        // Skip if already indexed (idempotent)
        if self.index.contains(id) {
            debug!(id, "Vector already indexed, skipping");
            return Ok(());
        }

        self.index
            .add(id, &vector)
            .map_err(|e| crate::Error::Embedding(e.to_string()))?;

        // Persist to disk so vectors survive server restarts
        self.index
            .save()
            .map_err(|e| crate::Error::Embedding(format!("Failed to save vector index: {e}")))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl VectorSearch for VectorBridge {
    async fn search(&self, query: &str, top_k: usize) -> crate::Result<Vec<(String, f32)>> {
        let vector = self
            .embedder
            .embed(query)
            .await
            .map_err(|e| crate::Error::Embedding(e.to_string()))?;

        let results = self
            .index
            .search(&vector, top_k)
            .map_err(|e| crate::Error::Embedding(e.to_string()))?;

        Ok(results.into_iter().map(|r| (r.id, r.score)).collect())
    }
}
