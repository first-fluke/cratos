//! Cratos Search - Vector Search and Semantic Indexing
//!
//! This crate provides vector search capabilities for Cratos:
//! - `VectorIndex`: HNSW-based vector index using usearch
//! - `IndexConfig`: Configuration for index parameters
//! - `SearchResult`: Search result with similarity scores
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Query Embedding (768 dims)                                 │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  VectorIndex (usearch HNSW)                                 │
//! │  ├─ ~/.cratos/vectors/executions.usearch                    │
//! │  └─ ~/.cratos/vectors/skills.usearch                        │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  SearchResult: [(id, score), ...]                          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use cratos_search::{VectorIndex, IndexConfig};
//!
//! // Create index for 768-dimensional embeddings (nomic-embed)
//! let config = IndexConfig::new(768);
//! let index = VectorIndex::new(config)?;
//!
//! // Add vectors
//! index.add("exec_123", &embedding)?;
//!
//! // Search
//! let results = index.search(&query_embedding, 5)?;
//! for result in results {
//!     println!("{}: score={:.3}", result.id, result.score);
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod index;

pub use error::{Error, Result};
pub use index::{IndexConfig, MetricType, SearchResult, VectorIndex};

/// Get the default vectors directory
pub fn default_vectors_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .map(|p| p.join(".cratos").join("vectors"))
        .unwrap_or_else(|| std::path::PathBuf::from(".cratos/vectors"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_vectors_dir() {
        let dir = default_vectors_dir();
        assert!(dir.to_string_lossy().contains("vectors"));
    }
}
