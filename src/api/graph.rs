//! Graph API - Knowledge graph visualization endpoints
//!
//! Provides REST endpoints for querying the Graph RAG data.

use std::sync::Arc;

use axum::{extract::Query, routing::get, Extension, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use cratos_memory::GraphMemory;

use super::config::ApiResponse;

/// Query parameters for graph data
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GraphQuery {
    /// Maximum number of nodes (default: 100)
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    100
}

/// Graph node for visualization
#[derive(Debug, Serialize, ToSchema)]
pub struct GraphNode {
    /// Entity ID
    pub id: String,
    /// Entity name (label)
    pub label: String,
    /// Entity kind (file, function, crate, tool, error, concept, config)
    pub kind: String,
    /// Number of mentions
    pub mention_count: u32,
}

/// Graph edge for visualization
#[derive(Debug, Serialize, ToSchema)]
pub struct GraphEdge {
    /// Source entity ID
    pub source: String,
    /// Target entity ID
    pub target: String,
    /// Edge weight (co-occurrence count)
    pub weight: u32,
}

/// Complete graph data for visualization
#[derive(Debug, Serialize, ToSchema)]
pub struct GraphData {
    /// Graph nodes (entities)
    pub nodes: Vec<GraphNode>,
    /// Graph edges (co-occurrences)
    pub edges: Vec<GraphEdge>,
}

/// Get graph data for visualization
#[utoipa::path(
    get,
    path = "/api/v1/graph",
    tag = "graph",
    params(GraphQuery),
    responses(
        (status = 200, description = "Graph data for visualization", body = GraphData),
        (status = 500, description = "Graph memory not initialized")
    )
)]
pub async fn get_graph(
    Extension(graph_memory): Extension<Option<Arc<GraphMemory>>>,
    Query(query): Query<GraphQuery>,
) -> Json<ApiResponse<GraphData>> {
    let graph_memory = match graph_memory {
        Some(gm) => gm,
        None => {
            return Json(ApiResponse::error("Graph memory not initialized"));
        }
    };

    // Fetch entities
    let entities = match graph_memory.list_entities(query.limit).await {
        Ok(e) => e,
        Err(e) => {
            return Json(ApiResponse::error(format!("Failed to fetch entities: {}", e)));
        }
    };

    let nodes: Vec<GraphNode> = entities
        .into_iter()
        .map(|e| GraphNode {
            id: e.id,
            label: e.name,
            kind: e.kind.to_string(),
            mention_count: e.mention_count,
        })
        .collect();

    // Fetch co-occurrences
    let cooccurrences = match graph_memory.list_cooccurrences(query.limit * 2).await {
        Ok(c) => c,
        Err(e) => {
            return Json(ApiResponse::error(format!(
                "Failed to fetch co-occurrences: {}",
                e
            )));
        }
    };

    let edges: Vec<GraphEdge> = cooccurrences
        .into_iter()
        .map(|(source, target, weight)| GraphEdge {
            source,
            target,
            weight,
        })
        .collect();

    Json(ApiResponse::success(GraphData { nodes, edges }))
}

/// Get graph statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct GraphStats {
    /// Total number of turns indexed
    pub turn_count: u32,
    /// Total number of entities
    pub entity_count: u32,
}

#[utoipa::path(
    get,
    path = "/api/v1/graph/stats",
    tag = "graph",
    responses(
        (status = 200, description = "Graph statistics", body = GraphStats),
        (status = 500, description = "Graph memory not initialized")
    )
)]
pub async fn get_graph_stats(
    Extension(graph_memory): Extension<Option<Arc<GraphMemory>>>,
) -> Json<ApiResponse<GraphStats>> {
    let graph_memory = match graph_memory {
        Some(gm) => gm,
        None => {
            return Json(ApiResponse::error("Graph memory not initialized"));
        }
    };

    let turn_count = graph_memory.turn_count().await.unwrap_or(0);
    let entity_count = graph_memory.entity_count().await.unwrap_or(0);

    Json(ApiResponse::success(GraphStats {
        turn_count,
        entity_count,
    }))
}

/// Create graph routes
pub fn graph_routes() -> Router {
    Router::new()
        .route("/api/v1/graph", get(get_graph))
        .route("/api/v1/graph/stats", get(get_graph_stats))
}
