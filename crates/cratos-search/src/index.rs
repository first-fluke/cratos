//! Vector index implementation using usearch
//!
//! This module provides HNSW-based vector indexing for semantic search.
//!
//! # Example
//!
//! ```ignore
//! use cratos_search::{VectorIndex, IndexConfig};
//!
//! let config = IndexConfig::new(768); // nomic-embed-text dimensions
//! let mut index = VectorIndex::new(config)?;
//!
//! // Add vectors
//! index.add("doc1", &embedding1)?;
//! index.add("doc2", &embedding2)?;
//!
//! // Search
//! let results = index.search(&query_embedding, 5)?;
//! ```

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use tracing::{debug, info, instrument, warn};
use usearch::ffi::{IndexOptions, MetricKind, ScalarKind};

/// Configuration for vector index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Vector dimensions
    pub dimensions: usize,
    /// Metric type (default: Cosine)
    pub metric: MetricType,
    /// Connectivity parameter (higher = more accurate but slower)
    pub connectivity: usize,
    /// Expansion factor for add operations
    pub expansion_add: usize,
    /// Expansion factor for search operations
    pub expansion_search: usize,
    /// Initial capacity
    pub capacity: usize,
}

/// Metric type for vector similarity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MetricType {
    /// Cosine similarity (default, best for text embeddings)
    #[default]
    Cosine,
    /// L2 (Euclidean) distance
    L2,
    /// Inner product
    InnerProduct,
}

impl MetricType {
    fn to_usearch(self) -> MetricKind {
        match self {
            MetricType::Cosine => MetricKind::Cos,
            MetricType::L2 => MetricKind::L2sq,
            MetricType::InnerProduct => MetricKind::IP,
        }
    }
}

impl IndexConfig {
    /// Create a new index configuration with default settings
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            metric: MetricType::Cosine,
            connectivity: 16,     // Good balance for most use cases
            expansion_add: 128,   // Higher for better index quality
            expansion_search: 64, // Higher for better recall
            capacity: 10_000,     // Initial capacity
        }
    }

    /// Set metric type
    pub fn with_metric(mut self, metric: MetricType) -> Self {
        self.metric = metric;
        self
    }

    /// Set connectivity parameter
    pub fn with_connectivity(mut self, connectivity: usize) -> Self {
        self.connectivity = connectivity;
        self
    }

    /// Set initial capacity
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }
}

/// Search result from vector index
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// External ID (string)
    pub id: String,
    /// Internal key (for usearch)
    pub key: u64,
    /// Similarity score (higher = more similar for cosine)
    pub score: f32,
    /// Distance (lower = more similar)
    pub distance: f32,
}

/// Vector index using HNSW algorithm via usearch
pub struct VectorIndex {
    /// usearch index
    index: usearch::Index,
    /// Mapping from external string ID to internal u64 key
    id_to_key: RwLock<HashMap<String, u64>>,
    /// Mapping from internal key to external string ID
    key_to_id: RwLock<HashMap<u64, String>>,
    /// Next available key
    next_key: AtomicU64,
    /// Index configuration
    config: IndexConfig,
    /// Path for persistence (if any)
    path: Option<std::path::PathBuf>,
}

impl VectorIndex {
    /// Create a new in-memory vector index
    pub fn new(config: IndexConfig) -> Result<Self> {
        let options = IndexOptions {
            dimensions: config.dimensions,
            metric: config.metric.to_usearch(),
            quantization: ScalarKind::F32,
            connectivity: config.connectivity,
            expansion_add: config.expansion_add,
            expansion_search: config.expansion_search,
            multi: false, // Single vector per key
        };

        let index = usearch::Index::new(&options)
            .map_err(|e| Error::Index(format!("Failed to create index: {}", e)))?;

        index
            .reserve(config.capacity)
            .map_err(|e| Error::Index(format!("Failed to reserve capacity: {}", e)))?;

        info!(
            "Created vector index: {} dims, {:?} metric, capacity {}",
            config.dimensions, config.metric, config.capacity
        );

        Ok(Self {
            index,
            id_to_key: RwLock::new(HashMap::new()),
            key_to_id: RwLock::new(HashMap::new()),
            next_key: AtomicU64::new(1),
            config,
            path: None,
        })
    }

    /// Open or create a persistent vector index
    pub fn open(path: &Path, config: IndexConfig) -> Result<Self> {
        let index_path = path.with_extension("usearch");
        let mapping_path = path.with_extension("mapping.json");

        // Try to load existing index
        if index_path.exists() && mapping_path.exists() {
            info!("Loading existing index from {}", index_path.display());
            return Self::load(&index_path, &mapping_path, config);
        }

        // Create new index
        let mut index = Self::new(config)?;
        index.path = Some(path.to_path_buf());

        Ok(index)
    }

    /// Load index from files
    fn load(index_path: &Path, mapping_path: &Path, config: IndexConfig) -> Result<Self> {
        let options = IndexOptions {
            dimensions: config.dimensions,
            metric: config.metric.to_usearch(),
            quantization: ScalarKind::F32,
            connectivity: config.connectivity,
            expansion_add: config.expansion_add,
            expansion_search: config.expansion_search,
            multi: false,
        };

        let index = usearch::Index::new(&options)
            .map_err(|e| Error::Index(format!("Failed to create index: {}", e)))?;

        let path_str = index_path
            .to_str()
            .ok_or_else(|| Error::Index("Invalid path encoding for index".to_string()))?;
        index
            .load(path_str)
            .map_err(|e| Error::Index(format!("Failed to load index: {}", e)))?;

        // Load ID mapping
        let mapping_content = std::fs::read_to_string(mapping_path)?;
        let mapping: IdMapping = serde_json::from_str(&mapping_content)
            .map_err(|e| Error::Serialization(format!("Failed to parse mapping: {}", e)))?;

        let id_to_key: HashMap<String, u64> = mapping.mappings.iter().cloned().collect();
        let key_to_id: HashMap<u64, String> =
            mapping.mappings.into_iter().map(|(k, v)| (v, k)).collect();
        let next_key = mapping.next_key;

        info!(
            "Loaded index with {} vectors from {}",
            id_to_key.len(),
            index_path.display()
        );

        Ok(Self {
            index,
            id_to_key: RwLock::new(id_to_key),
            key_to_id: RwLock::new(key_to_id),
            next_key: AtomicU64::new(next_key),
            config,
            path: Some(index_path.with_extension("")),
        })
    }

    /// Save index to disk
    #[instrument(skip(self))]
    pub fn save(&self) -> Result<()> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| Error::Index("No path set for persistent index".to_string()))?;

        let index_path = path.with_extension("usearch");
        let mapping_path = path.with_extension("mapping.json");

        // Save usearch index
        let index_path_str = index_path
            .to_str()
            .ok_or_else(|| Error::Index("Invalid path encoding for index".to_string()))?;
        self.index
            .save(index_path_str)
            .map_err(|e| Error::Index(format!("Failed to save index: {}", e)))?;

        // Save ID mapping
        let id_to_key = self.id_to_key.read().unwrap_or_else(|e| e.into_inner());
        let mapping = IdMapping {
            mappings: id_to_key.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            next_key: self.next_key.load(Ordering::SeqCst),
        };

        let mapping_json = serde_json::to_string_pretty(&mapping)
            .map_err(|e| Error::Serialization(format!("Failed to serialize mapping: {}", e)))?;
        std::fs::write(&mapping_path, mapping_json)?;

        info!(
            "Saved index with {} vectors to {}",
            id_to_key.len(),
            index_path.display()
        );

        Ok(())
    }

    /// Add a vector to the index
    #[instrument(skip(self, vector), fields(id = %id, vector_len = vector.len()))]
    pub fn add(&self, id: &str, vector: &[f32]) -> Result<()> {
        // Validate dimensions
        if vector.len() != self.config.dimensions {
            return Err(Error::DimensionMismatch {
                expected: self.config.dimensions,
                actual: vector.len(),
            });
        }

        // Check if ID already exists
        {
            let id_to_key = self.id_to_key.read().unwrap_or_else(|e| e.into_inner());
            if id_to_key.contains_key(id) {
                return Err(Error::AlreadyExists(id.to_string()));
            }
        }

        // Allocate new key
        let key = self.next_key.fetch_add(1, Ordering::SeqCst);

        // Auto-expand capacity if needed (loaded indexes may be at exact size)
        if self.index.size() >= self.index.capacity() {
            let new_cap = std::cmp::max(self.index.capacity() * 2, 64);
            self.index
                .reserve(new_cap)
                .map_err(|e| Error::Index(format!("Failed to expand capacity: {}", e)))?;
        }

        // Add to usearch index
        self.index
            .add(key, vector)
            .map_err(|e| Error::Index(format!("Failed to add vector: {}", e)))?;

        // Update mappings
        {
            let mut id_to_key = self.id_to_key.write().unwrap_or_else(|e| e.into_inner());
            let mut key_to_id = self.key_to_id.write().unwrap_or_else(|e| e.into_inner());
            id_to_key.insert(id.to_string(), key);
            key_to_id.insert(key, id.to_string());
        }

        debug!("Added vector for id={} with key={}", id, key);
        Ok(())
    }

    /// Update a vector in the index (remove + add)
    #[instrument(skip(self, vector), fields(id = %id))]
    pub fn update(&self, id: &str, vector: &[f32]) -> Result<()> {
        // Remove existing if present
        if self.contains(id) {
            self.remove(id)?;
        }

        // Add new
        self.add(id, vector)
    }

    /// Remove a vector from the index
    #[instrument(skip(self), fields(id = %id))]
    pub fn remove(&self, id: &str) -> Result<()> {
        let key = {
            let id_to_key = self.id_to_key.read().unwrap_or_else(|e| e.into_inner());
            *id_to_key
                .get(id)
                .ok_or_else(|| Error::NotFound(id.to_string()))?
        };

        // Remove from usearch index
        self.index
            .remove(key)
            .map_err(|e| Error::Index(format!("Failed to remove vector: {}", e)))?;

        // Update mappings
        {
            let mut id_to_key = self.id_to_key.write().unwrap_or_else(|e| e.into_inner());
            let mut key_to_id = self.key_to_id.write().unwrap_or_else(|e| e.into_inner());
            id_to_key.remove(id);
            key_to_id.remove(&key);
        }

        debug!("Removed vector for id={}", id);
        Ok(())
    }

    /// Search for similar vectors
    #[instrument(skip(self, query), fields(query_len = query.len(), top_k = top_k))]
    pub fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<SearchResult>> {
        // Validate dimensions
        if query.len() != self.config.dimensions {
            return Err(Error::DimensionMismatch {
                expected: self.config.dimensions,
                actual: query.len(),
            });
        }

        let results = self
            .index
            .search(query, top_k)
            .map_err(|e| Error::Search(format!("Search failed: {}", e)))?;

        let key_to_id = self.key_to_id.read().unwrap_or_else(|e| e.into_inner());
        let search_results: Vec<SearchResult> = results
            .keys
            .iter()
            .zip(results.distances.iter())
            .filter_map(|(key, distance)| {
                let id = key_to_id.get(key)?;
                // Convert distance to similarity score for cosine metric
                let score = match self.config.metric {
                    MetricType::Cosine => 1.0 - distance,
                    MetricType::L2 => 1.0 / (1.0 + distance),
                    MetricType::InnerProduct => *distance, // Already a similarity
                };
                Some(SearchResult {
                    id: id.clone(),
                    key: *key,
                    score,
                    distance: *distance,
                })
            })
            .collect();

        debug!("Found {} results for search query", search_results.len());
        Ok(search_results)
    }

    /// Check if an ID exists in the index
    pub fn contains(&self, id: &str) -> bool {
        let id_to_key = self.id_to_key.read().unwrap_or_else(|e| e.into_inner());
        id_to_key.contains_key(id)
    }

    /// Get the number of vectors in the index
    pub fn len(&self) -> usize {
        let id_to_key = self.id_to_key.read().unwrap_or_else(|e| e.into_inner());
        id_to_key.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the index configuration
    pub fn config(&self) -> &IndexConfig {
        &self.config
    }

    /// Get all IDs in the index
    pub fn ids(&self) -> Vec<String> {
        let id_to_key = self.id_to_key.read().unwrap_or_else(|e| e.into_inner());
        id_to_key.keys().cloned().collect()
    }

    /// Clear all vectors from the index
    pub fn clear(&self) -> Result<()> {
        // Clear usearch index by removing all keys
        let keys: Vec<u64> = {
            let key_to_id = self.key_to_id.read().unwrap_or_else(|e| e.into_inner());
            key_to_id.keys().cloned().collect()
        };

        for key in keys {
            if let Err(e) = self.index.remove(key) {
                warn!("Failed to remove key {} during clear: {}", key, e);
            }
        }

        // Clear mappings
        {
            let mut id_to_key = self.id_to_key.write().unwrap_or_else(|e| e.into_inner());
            let mut key_to_id = self.key_to_id.write().unwrap_or_else(|e| e.into_inner());
            id_to_key.clear();
            key_to_id.clear();
        }

        info!("Cleared vector index");
        Ok(())
    }
}

/// ID mapping for persistence
#[derive(Debug, Serialize, Deserialize)]
struct IdMapping {
    mappings: Vec<(String, u64)>,
    next_key: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_index() -> VectorIndex {
        VectorIndex::new(IndexConfig::new(4)).unwrap()
    }

    #[test]
    fn test_index_creation() {
        let index = create_test_index();
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
    }

    #[test]
    fn test_add_and_search() {
        let index = create_test_index();

        // Add vectors
        index.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();
        index.add("doc3", &[1.0, 1.0, 0.0, 0.0]).unwrap();

        assert_eq!(index.len(), 3);

        // Search
        let results = index.search(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn test_remove() {
        let index = create_test_index();

        index.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        assert!(index.contains("doc1"));
        assert_eq!(index.len(), 1);

        index.remove("doc1").unwrap();
        assert!(!index.contains("doc1"));
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_update() {
        let index = create_test_index();

        index.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.update("doc1", &[0.0, 1.0, 0.0, 0.0]).unwrap();

        let results = index.search(&[0.0, 1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn test_dimension_mismatch() {
        let index = create_test_index();

        let result = index.add("doc1", &[1.0, 0.0, 0.0]); // Wrong dimension
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_id() {
        let index = create_test_index();

        index.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        let result = index.add("doc1", &[0.0, 1.0, 0.0, 0.0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_clear() {
        let index = create_test_index();

        index.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();
        assert_eq!(index.len(), 2);

        index.clear().unwrap();
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_ids() {
        let index = create_test_index();

        index.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();

        let ids = index.ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"doc1".to_string()));
        assert!(ids.contains(&"doc2".to_string()));
    }
}
