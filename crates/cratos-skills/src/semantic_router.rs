//! Semantic skill router using vector embeddings
//!
//! This module provides hybrid skill routing combining:
//! - Traditional keyword/regex matching
//! - Semantic similarity using embeddings
//!
//! # Example
//!
//! ```ignore
//! use cratos_skills::{SemanticSkillRouter, SkillRegistry};
//!
//! let router = SemanticSkillRouter::new(
//!     registry,
//!     vector_index,
//!     embedding_provider,
//! );
//!
//! // Route using hybrid matching
//! let results = router.route("save the file").await?;
//! // Now matches "backup" skill even though "save" != "backup"
//! ```

use crate::error::{Error, Result};
use crate::registry::SkillRegistry;
use crate::router::{MatchReason, RouterConfig, RoutingResult, SkillRouter};
use crate::skill::Skill;
use async_trait::async_trait;
use cratos_search::{IndexConfig, VectorIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Trait for embedding provider (to avoid circular dependency)
#[async_trait]
pub trait SkillEmbedder: Send + Sync {
    /// Generate embedding for text
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple texts
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Get embedding dimensions
    fn dimensions(&self) -> usize;
}

/// Extended match reason including semantic matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticMatchReason {
    /// Matched via keyword
    Keyword(String),
    /// Matched via regex pattern
    Regex(String),
    /// Matched via intent classification
    Intent(String),
    /// Matched via semantic similarity
    Semantic {
        /// Similarity score
        similarity: f32,
    },
    /// Combined match (multiple reasons)
    Combined,
    /// Hybrid match (keyword + semantic)
    Hybrid {
        /// Keyword match score
        keyword_score: f32,
        /// Semantic similarity score
        semantic_score: f32,
    },
}

/// Result of semantic skill routing
#[derive(Debug, Clone)]
pub struct SemanticRoutingResult {
    /// The matched skill
    pub skill: Arc<Skill>,
    /// Combined score (0.0 - 1.0)
    pub score: f32,
    /// Keyword match score (0.0 - 1.0)
    pub keyword_score: f32,
    /// Semantic similarity score (0.0 - 1.0)
    pub semantic_score: f32,
    /// Reason for the match
    pub match_reason: SemanticMatchReason,
    /// Matched keywords (if any)
    pub matched_keywords: Vec<String>,
}

/// Configuration for semantic skill router
#[derive(Debug, Clone)]
pub struct SemanticRouterConfig {
    /// Base router configuration
    pub base_config: RouterConfig,
    /// Weight for keyword matching (0.0 - 1.0)
    pub keyword_weight: f32,
    /// Weight for semantic matching (0.0 - 1.0)
    pub semantic_weight: f32,
    /// Minimum semantic similarity to consider
    pub min_semantic_score: f32,
    /// Number of semantic candidates to consider
    pub semantic_top_k: usize,
    /// Whether to use semantic matching alone when no keyword match
    pub fallback_to_semantic: bool,
}

impl Default for SemanticRouterConfig {
    fn default() -> Self {
        Self {
            base_config: RouterConfig::default(),
            keyword_weight: 0.5,
            semantic_weight: 0.5,
            min_semantic_score: 0.4,
            semantic_top_k: 10,
            fallback_to_semantic: true,
        }
    }
}

/// Hybrid skill router combining keyword and semantic matching
pub struct SemanticSkillRouter<E: SkillEmbedder> {
    /// Skill registry
    registry: Arc<SkillRegistry>,
    /// Traditional keyword router
    keyword_router: RwLock<SkillRouter>,
    /// Vector index for skill embeddings
    index: Arc<RwLock<VectorIndex>>,
    /// Embedding provider
    embedder: Arc<E>,
    /// Configuration
    config: SemanticRouterConfig,
    /// Cache of skill IDs to names
    skill_id_to_name: RwLock<HashMap<String, String>>,
}

impl<E: SkillEmbedder> SemanticSkillRouter<E> {
    /// Create a new semantic skill router
    pub fn new(registry: Arc<SkillRegistry>, index: VectorIndex, embedder: Arc<E>) -> Self {
        let keyword_router = SkillRouter::new((*registry).clone());

        Self {
            registry,
            keyword_router: RwLock::new(keyword_router),
            index: Arc::new(RwLock::new(index)),
            embedder,
            config: SemanticRouterConfig::default(),
            skill_id_to_name: RwLock::new(HashMap::new()),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        registry: Arc<SkillRegistry>,
        index: VectorIndex,
        embedder: Arc<E>,
        config: SemanticRouterConfig,
    ) -> Self {
        let keyword_router =
            SkillRouter::with_config((*registry).clone(), config.base_config.clone());

        Self {
            registry,
            keyword_router: RwLock::new(keyword_router),
            index: Arc::new(RwLock::new(index)),
            embedder,
            config,
            skill_id_to_name: RwLock::new(HashMap::new()),
        }
    }

    /// Route input to skills using hybrid matching
    #[instrument(skip(self), fields(input_len = input.len()))]
    pub async fn route(&self, input: &str) -> Result<Vec<SemanticRoutingResult>> {
        // 1. Get keyword matches
        let keyword_results = {
            let mut router = self.keyword_router.write().await;
            router.route(input).await
        };

        // 2. Get semantic matches
        let semantic_results = self.semantic_search(input).await?;

        // 3. Merge and rank results
        let merged = self.merge_results(keyword_results, semantic_results).await;

        debug!(
            "Hybrid routing for '{}': {} results",
            truncate_text(input, 50),
            merged.len()
        );

        Ok(merged)
    }

    /// Get the best matching skill
    #[instrument(skip(self))]
    pub async fn route_best(&self, input: &str) -> Option<SemanticRoutingResult> {
        let results = self.route(input).await.ok()?;
        results
            .into_iter()
            .find(|r| r.score >= self.config.base_config.min_score)
    }

    /// Search for skills using semantic similarity
    async fn semantic_search(&self, input: &str) -> Result<Vec<(String, f32)>> {
        // Generate query embedding
        let query_embedding = self.embedder.embed(input).await?;

        // Search vector index
        let index = self.index.read().await;
        let results = index
            .search(&query_embedding, self.config.semantic_top_k)
            .map_err(|e| Error::Internal(format!("Semantic search failed: {}", e)))?;

        Ok(results
            .into_iter()
            .filter(|r| r.score >= self.config.min_semantic_score)
            .map(|r| (r.id, r.score))
            .collect())
    }

    /// Merge keyword and semantic results
    async fn merge_results(
        &self,
        keyword_results: Vec<RoutingResult>,
        semantic_results: Vec<(String, f32)>,
    ) -> Vec<SemanticRoutingResult> {
        let mut combined: HashMap<String, SemanticRoutingResult> = HashMap::new();

        // Process keyword results
        for kr in keyword_results {
            let skill_id = kr.skill.id.to_string();
            combined.insert(
                skill_id.clone(),
                SemanticRoutingResult {
                    skill: kr.skill,
                    score: kr.score * self.config.keyword_weight,
                    keyword_score: kr.score,
                    semantic_score: 0.0,
                    match_reason: convert_match_reason(kr.match_reason),
                    matched_keywords: kr.matched_keywords,
                },
            );
        }

        // Process semantic results
        let skill_id_to_name = self.skill_id_to_name.read().await;
        for (skill_id, semantic_score) in semantic_results {
            if let Some(existing) = combined.get_mut(&skill_id) {
                // Combine scores
                existing.semantic_score = semantic_score;
                let combined_score = existing.keyword_score * self.config.keyword_weight
                    + semantic_score * self.config.semantic_weight;
                existing.score = combined_score;
                existing.match_reason = SemanticMatchReason::Hybrid {
                    keyword_score: existing.keyword_score,
                    semantic_score,
                };
            } else if self.config.fallback_to_semantic {
                // Try to get skill from registry by name
                if let Some(skill_name) = skill_id_to_name.get(&skill_id) {
                    if let Some(skill) = self.registry.get_by_name(skill_name).await {
                        combined.insert(
                            skill_id,
                            SemanticRoutingResult {
                                skill,
                                score: semantic_score * self.config.semantic_weight,
                                keyword_score: 0.0,
                                semantic_score,
                                match_reason: SemanticMatchReason::Semantic {
                                    similarity: semantic_score,
                                },
                                matched_keywords: Vec::new(),
                            },
                        );
                    }
                }
            }
        }

        // Sort by combined score
        let mut results: Vec<_> = combined.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Index a skill for semantic search
    #[instrument(skip(self, skill), fields(skill_name = %skill.name))]
    pub async fn index_skill(&self, skill: &Skill) -> Result<()> {
        // Create text for embedding
        let text = create_skill_embedding_text(skill);
        let text = truncate_text(&text, 4096);

        // Generate embedding
        let embedding = self.embedder.embed(&text).await?;

        // Add to index
        let index = self.index.write().await;
        let id = skill.id.to_string();

        if index.contains(&id) {
            index
                .update(&id, &embedding)
                .map_err(|e| Error::Internal(format!("Failed to update skill index: {}", e)))?;
        } else {
            index
                .add(&id, &embedding)
                .map_err(|e| Error::Internal(format!("Failed to add skill to index: {}", e)))?;
        }

        // Update ID mapping
        {
            let mut mapping = self.skill_id_to_name.write().await;
            mapping.insert(id, skill.name.clone());
        }

        debug!("Indexed skill: {}", skill.name);
        Ok(())
    }

    /// Remove a skill from the index
    pub async fn remove_skill(&self, skill_id: &str) -> Result<()> {
        let index = self.index.write().await;
        if index.contains(skill_id) {
            index.remove(skill_id).map_err(|e| {
                Error::Internal(format!("Failed to remove skill from index: {}", e))
            })?;
        }

        // Update ID mapping
        {
            let mut mapping = self.skill_id_to_name.write().await;
            mapping.remove(skill_id);
        }

        Ok(())
    }

    /// Reindex all skills in the registry
    #[instrument(skip(self))]
    pub async fn reindex_all(&self) -> Result<usize> {
        info!("Reindexing all skills");

        let skills = self.registry.get_active().await;
        let total = skills.len();

        if total == 0 {
            info!("No skills to index");
            return Ok(0);
        }

        // Clear existing index
        {
            let index = self.index.write().await;
            index
                .clear()
                .map_err(|e| Error::Internal(format!("Failed to clear index: {}", e)))?;
        }

        // Create embedding texts
        let texts: Vec<String> = skills
            .iter()
            .map(|s| truncate_text(&create_skill_embedding_text(s), 4096))
            .collect();

        // Generate embeddings in batch
        let embeddings = self.embedder.embed_batch(&texts).await?;

        // Add to index
        let index = self.index.write().await;
        let mut mapping = self.skill_id_to_name.write().await;
        let mut indexed = 0;

        for (skill, embedding) in skills.iter().zip(embeddings.iter()) {
            let id = skill.id.to_string();
            if let Err(e) = index.add(&id, embedding) {
                warn!("Failed to index skill {}: {}", skill.name, e);
                continue;
            }
            mapping.insert(id, skill.name.clone());
            indexed += 1;
        }

        info!("Indexed {} skills", indexed);
        Ok(indexed)
    }

    /// Save the index to disk
    pub async fn save_index(&self) -> Result<()> {
        let index = self.index.read().await;
        index
            .save()
            .map_err(|e| Error::Internal(format!("Failed to save skill index: {}", e)))?;
        Ok(())
    }

    /// Get the number of indexed skills
    pub async fn index_size(&self) -> usize {
        let index = self.index.read().await;
        index.len()
    }
}

/// Create text for embedding from skill
fn create_skill_embedding_text(skill: &Skill) -> String {
    let mut parts = Vec::new();

    // Name and description
    parts.push(skill.name.clone());
    parts.push(skill.description.clone());

    // Keywords
    for keyword in &skill.trigger.keywords {
        parts.push(keyword.clone());
    }

    // Intents
    for intent in &skill.trigger.intents {
        parts.push(intent.clone());
    }

    // Step descriptions
    for step in &skill.steps {
        if let Some(ref desc) = step.description {
            parts.push(desc.clone());
        }
    }

    parts.join(" ")
}

/// Convert traditional match reason to semantic match reason
fn convert_match_reason(reason: MatchReason) -> SemanticMatchReason {
    match reason {
        MatchReason::Keyword(k) => SemanticMatchReason::Keyword(k),
        MatchReason::Regex(r) => SemanticMatchReason::Regex(r),
        MatchReason::Intent(i) => SemanticMatchReason::Intent(i),
        MatchReason::Combined => SemanticMatchReason::Combined,
    }
}

/// Truncate text to maximum length
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    let safe_end = text
        .char_indices()
        .take_while(|(i, _)| *i < max_len)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let truncated = &text[..safe_end];

    if let Some(last_space) = truncated.rfind(' ') {
        text[..last_space].to_string()
    } else {
        truncated.to_string()
    }
}

/// Create a default vector index for skills
pub fn create_skill_index(
    dimensions: usize,
    path: Option<&std::path::Path>,
) -> cratos_search::Result<VectorIndex> {
    let config = IndexConfig::new(dimensions).with_capacity(1_000);

    match path {
        Some(p) => VectorIndex::open(p, config),
        None => VectorIndex::new(config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{SkillCategory, SkillTrigger};

    #[test]
    fn test_create_skill_embedding_text() {
        let skill = Skill::new(
            "file_backup",
            "Backup files to storage",
            SkillCategory::Custom,
        )
        .with_trigger(
            SkillTrigger::with_keywords(vec!["backup".to_string(), "save".to_string()])
                .add_intent("file_operation"),
        );

        let text = create_skill_embedding_text(&skill);
        assert!(text.contains("file_backup"));
        assert!(text.contains("Backup files"));
        assert!(text.contains("backup"));
        assert!(text.contains("save"));
        assert!(text.contains("file_operation"));
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("hello world", 20), "hello world");
        // "hello world foo bar" is 19 chars, truncated to 12 gives "hello world " -> "hello world"
        assert_eq!(truncate_text("hello world foo bar", 12), "hello world");
        // Test exact length
        assert_eq!(truncate_text("hello", 5), "hello");
        // Test truncation without space
        assert_eq!(truncate_text("helloworld", 5), "hello");
    }

    #[test]
    fn test_semantic_router_config_default() {
        let config = SemanticRouterConfig::default();
        assert_eq!(config.keyword_weight, 0.5);
        assert_eq!(config.semantic_weight, 0.5);
        assert_eq!(config.min_semantic_score, 0.4);
        assert!(config.fallback_to_semantic);
    }
}
