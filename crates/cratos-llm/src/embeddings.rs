//! Embedding providers for vector search
//!
//! This module provides embedding generation for semantic search:
//! - `EmbeddingProvider` trait for abstraction
//! - `FastEmbedProvider` using fastembed-rs with nomic-embed-text-v1.5
//!
//! # Example
//!
//! ```ignore
//! use cratos_llm::embeddings::{EmbeddingProvider, FastEmbedProvider};
//!
//! let provider = FastEmbedProvider::new()?;
//! let embedding = provider.embed("Hello, world!").await?;
//! assert_eq!(embedding.len(), 768);
//! ```

use crate::error::{Error, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, instrument};

/// Trait for embedding providers
///
/// Embedding providers convert text into dense vector representations
/// suitable for semantic similarity search.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for a single text
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple texts (batch processing)
    ///
    /// Default implementation calls `embed` for each text sequentially.
    /// Providers may override this for more efficient batch processing.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            embeddings.push(self.embed(text).await?);
        }
        Ok(embeddings)
    }

    /// Get the embedding dimension
    fn dimensions(&self) -> usize;

    /// Get the provider name
    fn name(&self) -> &str;

    /// Get the model name
    fn model(&self) -> &str;
}

/// FastEmbed provider using nomic-embed-text-v1.5
///
/// This provider uses the fastembed-rs library for local, free embedding generation.
/// The nomic-embed-text-v1.5 model produces 768-dimensional vectors.
///
/// # Features
///
/// - **Free**: No API costs, runs locally
/// - **Fast**: Optimized ONNX runtime
/// - **Offline**: Works without internet after initial model download
/// - **High Quality**: Competitive with OpenAI embeddings
#[cfg(feature = "embeddings")]
pub struct FastEmbedProvider {
    model: std::sync::Arc<fastembed::TextEmbedding>,
    model_name: String,
    dimensions: usize,
}

#[cfg(feature = "embeddings")]
impl FastEmbedProvider {
    /// Create a new FastEmbed provider with default settings (nomic-embed-text-v1.5)
    ///
    /// # Errors
    ///
    /// Returns an error if the model fails to initialize. This may happen if:
    /// - The model download fails (first run only)
    /// - ONNX runtime initialization fails
    pub fn new() -> Result<Self> {
        Self::with_model(fastembed::EmbeddingModel::NomicEmbedTextV15)
    }

    /// Create a new FastEmbed provider with a specific model
    ///
    /// # Arguments
    ///
    /// * `model` - The fastembed model to use
    ///
    /// # Available Models
    ///
    /// - `NomicEmbedTextV15` - 768 dims, best quality (default)
    /// - `AllMiniLML6V2` - 384 dims, smaller/faster
    /// - `BGESmallENV15` - 384 dims, good for English
    pub fn with_model(model: fastembed::EmbeddingModel) -> Result<Self> {
        info!("Initializing FastEmbed provider with model: {:?}", model);

        let init_options =
            fastembed::InitOptions::new(model.clone()).with_show_download_progress(true);

        let text_embedding = fastembed::TextEmbedding::try_new(init_options)
            .map_err(|e| Error::Provider(format!("Failed to initialize FastEmbed: {}", e)))?;

        // Determine dimensions based on model
        let dimensions = match model {
            fastembed::EmbeddingModel::NomicEmbedTextV15 => 768,
            fastembed::EmbeddingModel::AllMiniLML6V2 => 384,
            fastembed::EmbeddingModel::BGESmallENV15 => 384,
            _ => 768, // Default assumption
        };

        let model_name = format!("{:?}", model);
        info!(
            "FastEmbed provider initialized: {} ({} dimensions)",
            model_name, dimensions
        );

        Ok(Self {
            model: std::sync::Arc::new(text_embedding),
            model_name,
            dimensions,
        })
    }

    /// Create provider with custom cache directory
    pub fn with_cache_dir(cache_dir: &std::path::Path) -> Result<Self> {
        info!(
            "Initializing FastEmbed with cache dir: {}",
            cache_dir.display()
        );

        let init_options =
            fastembed::InitOptions::new(fastembed::EmbeddingModel::NomicEmbedTextV15)
                .with_cache_dir(cache_dir.to_path_buf())
                .with_show_download_progress(true);

        let text_embedding = fastembed::TextEmbedding::try_new(init_options)
            .map_err(|e| Error::Provider(format!("Failed to initialize FastEmbed: {}", e)))?;

        Ok(Self {
            model: std::sync::Arc::new(text_embedding),
            model_name: "NomicEmbedTextV15".to_string(),
            dimensions: 768,
        })
    }
}

#[cfg(feature = "embeddings")]
#[async_trait]
impl EmbeddingProvider for FastEmbedProvider {
    #[instrument(skip(self, text), fields(text_len = text.len()))]
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let text = text.to_string();
        let model = self.model.clone();

        // Run embedding in blocking task (fastembed is synchronous)
        let embedding = tokio::task::spawn_blocking(move || {
            model
                .embed(vec![text], None)
                .map_err(|e| Error::Provider(format!("Embedding failed: {}", e)))
        })
        .await
        .map_err(|e| Error::Provider(format!("Task join error: {}", e)))??;

        let result = embedding
            .into_iter()
            .next()
            .ok_or_else(|| Error::Provider("Empty embedding result".to_string()))?;

        debug!("Generated embedding with {} dimensions", result.len());
        Ok(result)
    }

    #[instrument(skip(self, texts), fields(batch_size = texts.len()))]
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let texts = texts.to_vec();
        let model = self.model.clone();

        // Run batch embedding in blocking task
        let embeddings = tokio::task::spawn_blocking(move || {
            model
                .embed(texts, None)
                .map_err(|e| Error::Provider(format!("Batch embedding failed: {}", e)))
        })
        .await
        .map_err(|e| Error::Provider(format!("Task join error: {}", e)))??;

        debug!("Generated {} embeddings", embeddings.len());
        Ok(embeddings)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn name(&self) -> &str {
        "fastembed"
    }

    fn model(&self) -> &str {
        &self.model_name
    }
}

/// Wrapper for thread-safe embedding provider access
pub type SharedEmbeddingProvider = Arc<dyn EmbeddingProvider>;

/// Create a default embedding provider (FastEmbed if available, otherwise None)
#[cfg(feature = "embeddings")]
pub fn default_embedding_provider() -> Result<SharedEmbeddingProvider> {
    Ok(Arc::new(FastEmbedProvider::new()?))
}

/// Placeholder when embeddings feature is disabled
#[cfg(not(feature = "embeddings"))]
pub fn default_embedding_provider() -> Result<SharedEmbeddingProvider> {
    Err(Error::Provider(
        "Embeddings feature not enabled. Compile with --features embeddings".to_string(),
    ))
}

#[cfg(all(test, feature = "embeddings"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fastembed_provider_creation() {
        // Note: This test requires model download on first run
        let result = FastEmbedProvider::new();
        // We don't assert success as the model may not be available in CI
        if let Ok(provider) = result {
            assert_eq!(provider.dimensions(), 768);
            assert_eq!(provider.name(), "fastembed");
        }
    }

    #[tokio::test]
    async fn test_embed_single_text() {
        let provider = match FastEmbedProvider::new() {
            Ok(p) => p,
            Err(_) => return, // Skip if model not available
        };

        let embedding = provider.embed("Hello, world!").await.unwrap();
        assert_eq!(embedding.len(), 768);

        // Check embedding is normalized (roughly)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.1, "Embedding should be normalized");
    }

    #[tokio::test]
    async fn test_embed_batch() {
        let provider = match FastEmbedProvider::new() {
            Ok(p) => p,
            Err(_) => return, // Skip if model not available
        };

        let texts = vec![
            "Hello, world!".to_string(),
            "How are you?".to_string(),
            "Goodbye!".to_string(),
        ];

        let embeddings = provider.embed_batch(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 768);
        }
    }

    #[tokio::test]
    async fn test_semantic_similarity() {
        let provider = match FastEmbedProvider::new() {
            Ok(p) => p,
            Err(_) => return, // Skip if model not available
        };

        let emb1 = provider.embed("The cat sat on the mat").await.unwrap();
        let emb2 = provider.embed("A cat was sitting on a rug").await.unwrap();
        let emb3 = provider.embed("The weather is sunny today").await.unwrap();

        // Calculate cosine similarities
        let sim_12 = cosine_similarity(&emb1, &emb2);
        let sim_13 = cosine_similarity(&emb1, &emb3);

        // Similar sentences should have higher similarity
        assert!(
            sim_12 > sim_13,
            "Similar sentences should have higher similarity: {} vs {}",
            sim_12,
            sim_13
        );
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (norm_a * norm_b)
    }
}
