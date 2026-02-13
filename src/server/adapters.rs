//! Embedding adapter types
//!
//! Bridges between the EmbeddingProvider and domain-specific embedder traits.

use cratos_llm::SharedEmbeddingProvider;
use cratos_replay::SearchEmbedder;
use cratos_skills::SkillEmbedder;

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
