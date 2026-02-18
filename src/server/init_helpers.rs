//! Helper functions for server initialization
//!
//! Contains helper functions to reduce run() complexity.

use super::adapters::{EmbeddingAdapter, SkillEmbeddingAdapter};
use super::config::AppConfig;
use anyhow::{Context, Result};
use cratos_core::{admin_scopes, AuthStore};
use cratos_llm::{EmbeddingProvider, SharedEmbeddingProvider, TractEmbeddingProvider};
use cratos_memory::{GraphMemory, VectorBridge};
use cratos_replay::EventStore;
use cratos_search::{IndexConfig, VectorIndex};
use cratos_skills::{SemanticSkillRouter, SkillRegistry};
use cratos_tools::ToolRegistry;
use std::sync::Arc;
use tracing::{info, warn};

/// Type alias for optional execution searcher
pub type ExecutionSearcherOpt = Option<Arc<cratos_replay::ExecutionSearcher<EmbeddingAdapter>>>;
/// Type alias for optional semantic skill router
pub type SkillRouterOpt = Option<Arc<SemanticSkillRouter<SkillEmbeddingAdapter>>>;

/// Initialize vector search components
pub async fn init_vector_search(
    embedding_provider: &Option<SharedEmbeddingProvider>,
    vectors_dir: &std::path::Path,
    event_store: &Arc<EventStore>,
    skill_registry: &Arc<SkillRegistry>,
) -> Result<(ExecutionSearcherOpt, SkillRouterOpt)> {
    if let Some(ref embedder) = embedding_provider {
        std::fs::create_dir_all(vectors_dir).context("Failed to create vectors directory")?;

        let dimensions = embedder.dimensions();

        let exec_index_path = vectors_dir.join("executions");
        let exec_index = match VectorIndex::open(&exec_index_path, IndexConfig::new(dimensions)) {
            Ok(idx) => {
                info!(
                    "Execution vector index loaded from {}",
                    exec_index_path.display()
                );
                idx
            }
            Err(e) => {
                warn!("Failed to load execution index, creating new: {}", e);
                VectorIndex::open(&exec_index_path, IndexConfig::new(dimensions))
                    .context("Failed to create execution vector index")?
            }
        };

        let skill_index_path = vectors_dir.join("skills");
        let skill_index = match VectorIndex::open(&skill_index_path, IndexConfig::new(dimensions)) {
            Ok(idx) => {
                info!(
                    "Skill vector index loaded from {}",
                    skill_index_path.display()
                );
                idx
            }
            Err(e) => {
                warn!("Failed to load skill index, creating new: {}", e);
                VectorIndex::open(&skill_index_path, IndexConfig::new(dimensions))
                    .context("Failed to create skill vector index")?
            }
        };

        let exec_embedder = Arc::new(EmbeddingAdapter {
            provider: embedder.clone(),
        });
        let exec_searcher =
            cratos_replay::ExecutionSearcher::new(event_store.clone(), exec_index, exec_embedder);
        info!("Execution searcher initialized");

        let skill_embedder = Arc::new(SkillEmbeddingAdapter {
            provider: embedder.clone(),
        });
        let skill_router =
            SemanticSkillRouter::new(skill_registry.clone(), skill_index, skill_embedder);

        let indexed = skill_router.reindex_all().await.unwrap_or(0);
        info!(
            "Semantic skill router initialized with {} indexed skills",
            indexed
        );

        Ok((Some(Arc::new(exec_searcher)), Some(Arc::new(skill_router))))
    } else {
        info!("Vector search not available, using keyword-only routing");
        Ok((None, None))
    }
}

/// Initialize Graph RAG memory
pub async fn init_graph_memory(
    data_dir: &std::path::Path,
    vectors_dir: &std::path::Path,
    embedding_provider: &Option<SharedEmbeddingProvider>,
    tool_registry: &mut ToolRegistry,
) -> Option<Arc<GraphMemory>> {
    let memory_db_path = data_dir.join("memory.db");
    match GraphMemory::from_path(&memory_db_path).await {
        Ok(gm) => {
            let gm = if let Some(ref embedder) = embedding_provider {
                let dimensions = embedder.dimensions();
                // Turn embedding index
                let memory_index_path = vectors_dir.join("memory");
                let gm = match VectorIndex::open(&memory_index_path, IndexConfig::new(dimensions)) {
                    Ok(idx) => {
                        let bridge = Arc::new(VectorBridge::new(embedder.clone(), Arc::new(idx)));
                        info!("Graph RAG memory initialized with embedding search");
                        gm.with_vector_bridge(bridge)
                    }
                    Err(e) => {
                        warn!("Failed to open memory vector index: {e}, using graph-only");
                        gm
                    }
                };
                // Explicit memory embedding index (separate HNSW)
                let explicit_index_path = vectors_dir.join("explicit");
                match VectorIndex::open(&explicit_index_path, IndexConfig::new(dimensions)) {
                    Ok(idx) => {
                        let bridge = Arc::new(VectorBridge::new(embedder.clone(), Arc::new(idx)));
                        info!("Explicit memory vector index initialized");
                        gm.with_explicit_vector_bridge(bridge)
                    }
                    Err(e) => {
                        warn!("Failed to open explicit memory vector index: {e}");
                        gm
                    }
                }
            } else {
                info!("Graph RAG memory initialized (graph-only, no embeddings)");
                gm
            };
            let gm = Arc::new(gm);

            // Register memory tool (explicit save/recall)
            tool_registry.register(Arc::new(crate::tools::MemoryTool::new(Arc::clone(&gm))));
            // Backfill: embed any explicit memories missing from vector index
            if let Err(e) = gm.reindex_explicit_memories().await {
                warn!("Failed to reindex explicit memories: {e}");
            }

            Some(gm)
        }
        Err(e) => {
            warn!("Failed to initialize Graph RAG memory: {e}");
            None
        }
    }
}

/// Initialize authentication store
pub fn init_auth(config: &AppConfig) -> Arc<AuthStore> {
    let auth_enabled = config.server.auth.enabled;
    let auth_store = Arc::new(AuthStore::new(auth_enabled));

    if auth_enabled && config.server.auth.auto_generate_key && auth_store.active_key_count() == 0 {
        // Auto-generate admin API key on first run
        match auth_store.generate_api_key("admin", admin_scopes(), "auto-generated admin key") {
            Ok((key, _hash)) => {
                info!("==========================================================");
                info!("  AUTO-GENERATED ADMIN API KEY (save this, shown once!):");
                info!("  {}", key.expose());
                info!("==========================================================");
            }
            Err(e) => {
                warn!("Failed to auto-generate API key: {}", e);
            }
        }
    }

    if auth_enabled {
        info!("Authentication ENABLED - API key required for all endpoints");
    } else {
        warn!("SECURITY: Authentication disabled — all API endpoints open (development only). Enable [server.auth] enabled = true for production.");
    }

    auth_store
}

/// Initialize embedding provider
pub fn init_embedding_provider(config: &AppConfig) -> Option<SharedEmbeddingProvider> {
    if config.vector_search.enabled {
        match TractEmbeddingProvider::new() {
            Ok(provider) => {
                info!(
                    "Embedding provider initialized: {} ({} dimensions)",
                    provider.name(),
                    provider.dimensions()
                );
                Some(Arc::new(provider))
            }
            Err(e) => {
                warn!(
                    "Failed to initialize embedding provider: {}. Semantic search disabled.",
                    e
                );
                None
            }
        }
    } else {
        info!("Vector search disabled by configuration");
        None
    }
}
