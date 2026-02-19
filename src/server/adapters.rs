//! Embedding adapter types
//!
//! Bridges between the EmbeddingProvider and domain-specific embedder traits.

use cratos_llm::SharedEmbeddingProvider;
use cratos_replay::SearchEmbedder;
use cratos_skills::{SemanticSkillRouter, SkillEmbedder};
use std::sync::Arc;

/// Adapter to use EmbeddingProvider as SearchEmbedder
pub struct EmbeddingAdapter {
    pub(crate) provider: SharedEmbeddingProvider,
}

#[async_trait::async_trait]
impl SearchEmbedder for EmbeddingAdapter {
    async fn embed(&self, text: &str) -> cratos_replay::Result<Vec<f32>> {
        self.provider
            .embed(text)
            .await
            .map_err(|e| cratos_replay::Error::Database(format!("Embedding failed: {}", e)))
    }

    async fn embed_batch(&self, texts: &[String]) -> cratos_replay::Result<Vec<Vec<f32>>> {
        self.provider
            .embed_batch(texts)
            .await
            .map_err(|e| cratos_replay::Error::Database(format!("Batch embedding failed: {}", e)))
    }

    fn dimensions(&self) -> usize {
        self.provider.dimensions()
    }
}

/// Adapter to use EmbeddingProvider as SkillEmbedder
pub struct SkillEmbeddingAdapter {
    pub(crate) provider: SharedEmbeddingProvider,
}

#[async_trait::async_trait]
impl SkillEmbedder for SkillEmbeddingAdapter {
    async fn embed(&self, text: &str) -> cratos_skills::Result<Vec<f32>> {
        self.provider
            .embed(text)
            .await
            .map_err(|e| cratos_skills::Error::Internal(format!("Embedding failed: {}", e)))
    }

    async fn embed_batch(&self, texts: &[String]) -> cratos_skills::Result<Vec<Vec<f32>>> {
        self.provider
            .embed_batch(texts)
            .await
            .map_err(|e| cratos_skills::Error::Internal(format!("Batch embedding failed: {}", e)))
    }

    fn dimensions(&self) -> usize {
        self.provider.dimensions()
    }
}

/// Adapter to connect SemanticSkillRouter to Orchestrator's SkillRouting trait
pub struct SkillRouterAdapter(pub Arc<SemanticSkillRouter<SkillEmbeddingAdapter>>);

#[async_trait::async_trait]
impl cratos_core::SkillRouting for SkillRouterAdapter {
    async fn route_best(&self, input: &str) -> Option<cratos_core::SkillMatch> {
        self.0
            .route_best(input)
            .await
            .map(|m| cratos_core::SkillMatch {
                skill_id: m.skill.id,
                skill_name: m.skill.name.clone(),
                description: m.skill.description.clone(),
                score: m.score,
            })
    }
}
