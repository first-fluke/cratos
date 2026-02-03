//! Semantic search for execution history
//!
//! This module provides natural language search over execution history
//! using vector embeddings for semantic similarity.
//!
//! # Example
//!
//! ```ignore
//! use cratos_replay::{ExecutionSearcher, EventStore};
//! use cratos_llm::FastEmbedProvider;
//! use cratos_search::VectorIndex;
//!
//! let searcher = ExecutionSearcher::new(
//!     event_store,
//!     vector_index,
//!     embedding_provider,
//! );
//!
//! // Search for executions
//! let results = searcher.search("배포 작업", 5).await?;
//! ```

use crate::error::{Error, Result};
use crate::event::Execution;
use crate::store::EventStore;
use async_trait::async_trait;
use cratos_search::{IndexConfig, VectorIndex};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Trait for embedding provider (to avoid circular dependency)
#[async_trait]
pub trait SearchEmbedder: Send + Sync {
    /// Generate embedding for text
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple texts
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Get embedding dimensions
    fn dimensions(&self) -> usize;
}

/// Search result for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSearchResult {
    /// Execution ID
    pub execution_id: String,
    /// Similarity score (0.0 - 1.0)
    pub score: f32,
    /// Matched execution (loaded on demand)
    #[serde(skip)]
    pub execution: Option<Execution>,
    /// Snippet of matching text
    pub snippet: Option<String>,
}

/// Configuration for execution searcher
#[derive(Debug, Clone)]
pub struct SearcherConfig {
    /// Default number of results
    pub default_top_k: usize,
    /// Maximum number of results
    pub max_top_k: usize,
    /// Minimum similarity score threshold
    pub min_score: f32,
    /// Maximum text length for embedding
    pub max_text_length: usize,
    /// Batch size for reindexing
    pub batch_size: usize,
}

impl Default for SearcherConfig {
    fn default() -> Self {
        Self {
            default_top_k: 10,
            max_top_k: 100,
            min_score: 0.3,
            max_text_length: 8192,
            batch_size: 100,
        }
    }
}

/// Semantic search over execution history
pub struct ExecutionSearcher<E: SearchEmbedder> {
    /// Event store for execution data
    store: Arc<EventStore>,
    /// Vector index for embeddings
    index: Arc<RwLock<VectorIndex>>,
    /// Embedding provider
    embedder: Arc<E>,
    /// Configuration
    config: SearcherConfig,
}

impl<E: SearchEmbedder> ExecutionSearcher<E> {
    /// Create a new execution searcher
    pub fn new(store: Arc<EventStore>, index: VectorIndex, embedder: Arc<E>) -> Self {
        Self {
            store,
            index: Arc::new(RwLock::new(index)),
            embedder,
            config: SearcherConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        store: Arc<EventStore>,
        index: VectorIndex,
        embedder: Arc<E>,
        config: SearcherConfig,
    ) -> Self {
        Self {
            store,
            index: Arc::new(RwLock::new(index)),
            embedder,
            config,
        }
    }

    /// Search for executions matching a natural language query
    #[instrument(skip(self), fields(query_len = query.len()))]
    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<ExecutionSearchResult>> {
        let top_k = top_k.min(self.config.max_top_k);

        // Generate query embedding
        let query_embedding = self.embedder.embed(query).await?;

        // Search vector index
        let index = self.index.read().await;
        let vector_results = index
            .search(&query_embedding, top_k)
            .map_err(|e| Error::Database(format!("Vector search failed: {}", e)))?;

        // Convert to execution search results
        let mut results = Vec::with_capacity(vector_results.len());
        for vr in vector_results {
            if vr.score < self.config.min_score {
                continue;
            }

            let execution_id = vr.id.clone();

            // Try to load execution details
            let execution = match uuid::Uuid::parse_str(&execution_id) {
                Ok(uuid) => self.store.get_execution(uuid).await.ok(),
                Err(_) => None,
            };

            let snippet = execution
                .as_ref()
                .map(|e| truncate_text(&e.input_text, 200));

            results.push(ExecutionSearchResult {
                execution_id,
                score: vr.score,
                execution,
                snippet,
            });
        }

        debug!(
            "Search '{}' returned {} results",
            truncate_text(query, 50),
            results.len()
        );

        Ok(results)
    }

    /// Index a single execution
    #[instrument(skip(self, execution), fields(execution_id = %execution.id))]
    pub async fn index_execution(&self, execution: &Execution) -> Result<()> {
        // Create text for embedding
        let text = create_embedding_text(execution);
        let text = truncate_text(&text, self.config.max_text_length);

        // Generate embedding
        let embedding = self.embedder.embed(&text).await?;

        // Add to index
        let index = self.index.write().await;
        let id = execution.id.to_string();

        // Update if exists, otherwise add
        if index.contains(&id) {
            index
                .update(&id, &embedding)
                .map_err(|e| Error::Database(format!("Failed to update index: {}", e)))?;
        } else {
            index
                .add(&id, &embedding)
                .map_err(|e| Error::Database(format!("Failed to add to index: {}", e)))?;
        }

        debug!("Indexed execution {}", execution.id);
        Ok(())
    }

    /// Remove an execution from the index
    #[instrument(skip(self))]
    pub async fn remove_execution(&self, execution_id: &str) -> Result<()> {
        let index = self.index.write().await;
        if index.contains(execution_id) {
            index
                .remove(execution_id)
                .map_err(|e| Error::Database(format!("Failed to remove from index: {}", e)))?;
            debug!("Removed execution {} from index", execution_id);
        }
        Ok(())
    }

    /// Reindex all executions
    #[instrument(skip(self))]
    pub async fn reindex_all(&self) -> Result<usize> {
        info!("Starting full reindex of executions");

        // Get all executions
        let executions = self.store.list_recent_executions(10000).await?;
        let total = executions.len();

        if total == 0 {
            info!("No executions to index");
            return Ok(0);
        }

        // Clear existing index
        {
            let index = self.index.write().await;
            index
                .clear()
                .map_err(|e| Error::Database(format!("Failed to clear index: {}", e)))?;
        }

        // Process in batches
        let mut indexed = 0;
        for chunk in executions.chunks(self.config.batch_size) {
            // Create embedding texts
            let texts: Vec<String> = chunk
                .iter()
                .map(|e| truncate_text(&create_embedding_text(e), self.config.max_text_length))
                .collect();

            // Generate embeddings in batch
            let embeddings = self.embedder.embed_batch(&texts).await?;

            // Add to index
            let index = self.index.write().await;
            for (execution, embedding) in chunk.iter().zip(embeddings.iter()) {
                let id = execution.id.to_string();
                if let Err(e) = index.add(&id, embedding) {
                    warn!("Failed to index execution {}: {}", id, e);
                    continue;
                }
                indexed += 1;
            }

            debug!("Indexed {}/{} executions", indexed, total);
        }

        info!("Reindex complete: {} executions indexed", indexed);
        Ok(indexed)
    }

    /// Save the index to disk
    pub async fn save_index(&self) -> Result<()> {
        let index = self.index.read().await;
        index
            .save()
            .map_err(|e| Error::Database(format!("Failed to save index: {}", e)))?;
        Ok(())
    }

    /// Get the number of indexed executions
    pub async fn index_size(&self) -> usize {
        let index = self.index.read().await;
        index.len()
    }
}

/// Create text for embedding from execution
fn create_embedding_text(execution: &Execution) -> String {
    let mut parts = Vec::new();

    // Include input text
    parts.push(execution.input_text.clone());

    // Include output if available
    if let Some(ref output) = execution.output_text {
        parts.push(output.clone());
    }

    // Include metadata summary if available
    if let Some(obj) = execution.metadata.as_object() {
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                parts.push(format!("{}: {}", key, s));
            }
        }
    }

    parts.join(" ")
}

/// Truncate text to maximum length, preserving word boundaries
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    // Find last space before limit
    let truncated = &text[..max_len];
    if let Some(last_space) = truncated.rfind(' ') {
        format!("{}...", &text[..last_space])
    } else {
        format!("{}...", truncated)
    }
}

/// Create a default vector index for executions
pub fn create_execution_index(
    dimensions: usize,
    path: Option<&std::path::Path>,
) -> cratos_search::Result<VectorIndex> {
    let config = IndexConfig::new(dimensions).with_capacity(10_000);

    match path {
        Some(p) => VectorIndex::open(p, config),
        None => VectorIndex::new(config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("hello", 10), "hello");
        assert_eq!(truncate_text("hello world", 8), "hello...");
        assert_eq!(truncate_text("abcdefghij", 5), "abcde...");
    }

    #[test]
    fn test_create_embedding_text() {
        let execution = Execution::new("telegram", "123", "user1", "Deploy to production");
        let text = create_embedding_text(&execution);
        assert!(text.contains("Deploy to production"));
    }
}
