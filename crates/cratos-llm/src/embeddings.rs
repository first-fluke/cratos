//! Embedding providers for vector search
//!
//! This module provides embedding generation for semantic search:
//! - `EmbeddingProvider` trait for abstraction
//! - `TractEmbeddingProvider` using tract (pure Rust ONNX runtime)
//!
//! # Example
//!
//! ```ignore
//! use cratos_llm::embeddings::{EmbeddingProvider, TractEmbeddingProvider};
//!
//! let provider = TractEmbeddingProvider::new()?;
//! let embedding = provider.embed("Hello, world!").await?;
//! assert_eq!(embedding.len(), 384);
//! ```

use crate::error::{Error, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, instrument};
#[cfg(feature = "embeddings")]
use tract_onnx::prelude::{Framework, InferenceModelExt};

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

/// Tract-based embedding provider using pure Rust ONNX runtime
///
/// This provider uses tract-onnx (pure Rust, no C++ dependencies) for local embedding generation.
/// Default model is sentence-transformers/all-MiniLM-L6-v2 which produces 384-dimensional vectors.
///
/// # Features
///
/// - **Free**: No API costs, runs locally
/// - **Pure Rust**: No C++ linking issues, works on all platforms
/// - **Offline**: Works without internet after initial model download
/// - **Lightweight**: Small binary size compared to ONNX Runtime
#[cfg(feature = "embeddings")]
#[allow(clippy::type_complexity)]
pub struct TractEmbeddingProvider {
    model: Arc<
        tract_onnx::prelude::SimplePlan<
            tract_onnx::prelude::TypedFact,
            Box<dyn tract_onnx::prelude::TypedOp>,
            tract_onnx::prelude::Graph<
                tract_onnx::prelude::TypedFact,
                Box<dyn tract_onnx::prelude::TypedOp>,
            >,
        >,
    >,
    tokenizer: Arc<tokenizers::Tokenizer>,
    model_name: String,
    dimensions: usize,
    max_length: usize,
}

#[cfg(feature = "embeddings")]
impl TractEmbeddingProvider {
    /// Create a new Tract embedding provider with default settings (all-MiniLM-L6-v2, 384 dims)
    ///
    /// # Errors
    ///
    /// Returns an error if the model fails to initialize. This may happen if:
    /// - The model download fails (first run only)
    /// - ONNX model loading fails
    pub fn new() -> Result<Self> {
        Self::with_model("sentence-transformers/all-MiniLM-L6-v2", 384, 256)
    }

    /// Create a new Tract embedding provider with a specific model
    ///
    /// # Arguments
    ///
    /// * `model_id` - HuggingFace model ID (must have ONNX files)
    /// * `dimensions` - Output embedding dimensions
    /// * `max_length` - Maximum sequence length
    ///
    /// # Available Models
    ///
    /// - `sentence-transformers/all-MiniLM-L6-v2` - 384 dims, fast (default)
    /// - `sentence-transformers/all-MiniLM-L12-v2` - 384 dims, balanced
    /// - `sentence-transformers/paraphrase-MiniLM-L6-v2` - 384 dims
    pub fn with_model(model_id: &str, dimensions: usize, max_length: usize) -> Result<Self> {
        info!(
            "Initializing Tract embedding provider with model: {}",
            model_id
        );

        // Download model files from HuggingFace
        let api = hf_hub::api::sync::Api::new()
            .map_err(|e| Error::Provider(format!("Failed to create HF API: {}", e)))?;

        let repo = api.model(model_id.to_string());

        // Download ONNX model
        let model_path = repo
            .get("model.onnx")
            .or_else(|_| repo.get("onnx/model.onnx"))
            .map_err(|e| Error::Provider(format!("Failed to download ONNX model: {}", e)))?;

        // Download tokenizer
        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| Error::Provider(format!("Failed to download tokenizer: {}", e)))?;

        // Load ONNX model with tract
        let model = tract_onnx::onnx()
            .model_for_path(&model_path)
            .map_err(|e| Error::Provider(format!("Failed to load ONNX model: {}", e)))?
            .into_optimized()
            .map_err(|e| Error::Provider(format!("Failed to optimize model: {}", e)))?
            .into_runnable()
            .map_err(|e| Error::Provider(format!("Failed to make model runnable: {}", e)))?;

        // Load tokenizer
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| Error::Provider(format!("Failed to load tokenizer: {}", e)))?;

        info!(
            "Tract embedding provider initialized: {} ({} dimensions)",
            model_id, dimensions
        );

        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            model_name: model_id.to_string(),
            dimensions,
            max_length,
        })
    }

    /// Tokenize text and return input tensors (input_ids, attention_mask, token_type_ids).
    fn tokenize(
        &self,
        text: &str,
    ) -> Result<(
        tract_onnx::prelude::Tensor,
        tract_onnx::prelude::Tensor,
        tract_onnx::prelude::Tensor,
    )> {
        use tract_onnx::prelude::*;

        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| Error::Provider(format!("Tokenization failed: {}", e)))?;

        let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mut attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let mut token_type_ids: Vec<i64> = encoding
            .get_type_ids()
            .iter()
            .map(|&x| x as i64)
            .collect();

        // Truncate if needed
        if input_ids.len() > self.max_length {
            input_ids.truncate(self.max_length);
            attention_mask.truncate(self.max_length);
            token_type_ids.truncate(self.max_length);
        }

        let seq_len = input_ids.len();

        // Create tensors with shape [1, seq_len]
        let input_ids_tensor: Tensor =
            tract_ndarray::Array2::from_shape_vec((1, seq_len), input_ids)
                .map_err(|e| Error::Provider(format!("Failed to create input_ids tensor: {}", e)))?
                .into();

        let attention_mask_tensor: Tensor =
            tract_ndarray::Array2::from_shape_vec((1, seq_len), attention_mask)
                .map_err(|e| {
                    Error::Provider(format!("Failed to create attention_mask tensor: {}", e))
                })?
                .into();

        let token_type_ids_tensor: Tensor =
            tract_ndarray::Array2::from_shape_vec((1, seq_len), token_type_ids)
                .map_err(|e| {
                    Error::Provider(format!(
                        "Failed to create token_type_ids tensor: {}",
                        e
                    ))
                })?
                .into();

        Ok((input_ids_tensor, attention_mask_tensor, token_type_ids_tensor))
    }

    /// Mean pooling over token embeddings
    fn mean_pooling(
        &self,
        token_embeddings: &tract_onnx::prelude::Tensor,
        attention_mask: &tract_onnx::prelude::Tensor,
    ) -> Result<Vec<f32>> {
        let embeddings = token_embeddings
            .to_array_view::<f32>()
            .map_err(|e| Error::Provider(format!("Failed to convert embeddings: {}", e)))?;

        let mask = attention_mask
            .to_array_view::<i64>()
            .map_err(|e| Error::Provider(format!("Failed to convert mask: {}", e)))?;

        // embeddings shape: [1, seq_len, hidden_size]
        // mask shape: [1, seq_len]
        let shape = embeddings.shape();
        let seq_len = shape[1];
        let hidden_size = shape[2];

        let mut sum = vec![0.0f32; hidden_size];
        let mut count = 0.0f32;

        for i in 0..seq_len {
            let mask_val = mask[[0, i]] as f32;
            if mask_val > 0.0 {
                for j in 0..hidden_size {
                    sum[j] += embeddings[[0, i, j]] * mask_val;
                }
                count += mask_val;
            }
        }

        // Avoid division by zero
        if count > 0.0 {
            for val in &mut sum {
                *val /= count;
            }
        }

        // L2 normalize
        let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut sum {
                *val /= norm;
            }
        }

        Ok(sum)
    }
}

#[cfg(feature = "embeddings")]
#[async_trait]
impl EmbeddingProvider for TractEmbeddingProvider {
    #[instrument(skip(self, text), fields(text_len = text.len()))]
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        use tract_onnx::prelude::*;

        let (input_ids, attention_mask, token_type_ids) = self.tokenize(text)?;
        let attention_mask_clone = attention_mask.clone();

        // Run inference (model expects: input_ids, attention_mask, token_type_ids)
        let outputs = self
            .model
            .run(tvec!(
                input_ids.into(),
                attention_mask.into(),
                token_type_ids.into()
            ))
            .map_err(|e| Error::Provider(format!("Inference failed: {}", e)))?;

        // Get token embeddings (first output)
        let token_embeddings = &outputs[0];

        // Mean pooling
        let embedding = self.mean_pooling(token_embeddings, &attention_mask_clone)?;

        debug!("Generated embedding with {} dimensions", embedding.len());
        Ok(embedding)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn name(&self) -> &str {
        "tract"
    }

    fn model(&self) -> &str {
        &self.model_name
    }
}

/// Wrapper for thread-safe embedding provider access
pub type SharedEmbeddingProvider = Arc<dyn EmbeddingProvider>;

/// Create a default embedding provider (Tract if available, otherwise None)
#[cfg(feature = "embeddings")]
pub fn default_embedding_provider() -> Result<SharedEmbeddingProvider> {
    Ok(Arc::new(TractEmbeddingProvider::new()?))
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
    async fn test_tract_provider_creation() {
        // Note: This test requires model download on first run
        let result = TractEmbeddingProvider::new();
        // We don't assert success as the model may not be available in CI
        if let Ok(provider) = result {
            assert_eq!(provider.dimensions(), 384);
            assert_eq!(provider.name(), "tract");
        }
    }

    #[tokio::test]
    async fn test_embed_single_text() {
        let provider = match TractEmbeddingProvider::new() {
            Ok(p) => p,
            Err(_) => return, // Skip if model not available
        };

        let embedding = provider.embed("Hello, world!").await.unwrap();
        assert_eq!(embedding.len(), 384);

        // Check embedding is normalized (roughly)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.1, "Embedding should be normalized");
    }

    #[tokio::test]
    async fn test_embed_batch() {
        let provider = match TractEmbeddingProvider::new() {
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
            assert_eq!(emb.len(), 384);
        }
    }

    #[tokio::test]
    async fn test_semantic_similarity() {
        let provider = match TractEmbeddingProvider::new() {
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
