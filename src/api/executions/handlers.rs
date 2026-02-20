use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use cratos_replay::{EventStore, ExecutionViewer, ReplayOptions};

use super::super::config::ApiResponse;
use super::types::{
    EventSummary, ExecutionDetail, ExecutionStats, ExecutionSummary, ListExecutionsQuery,
};
use crate::middleware::auth::RequireAuth;

/// List recent executions (requires authentication)
#[utoipa::path(
    get,
    path = "/api/v1/executions",
    tag = "executions",
    params(ListExecutionsQuery),
    responses(
        (status = 200, description = "List of executions", body = Vec<ExecutionSummary>),
        (status = 401, description = "Unauthorized")
    ),
    security(("api_key" = []))
)]
pub async fn list_executions(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
    Query(query): Query<ListExecutionsQuery>,
) -> Json<ApiResponse<Vec<ExecutionSummary>>> {
    let limit = query.limit.clamp(1, 200);

    // Use channel-specific query if channel filter provided
    let executions = if let Some(ref channel) = query.channel {
        match store
            .list_executions_by_channel(channel, "", limit, 0)
            .await
        {
            Ok(execs) => execs,
            Err(e) => {
                return Json(ApiResponse::error(format!(
                    "Failed to list executions: {}",
                    e
                )));
            }
        }
    } else {
        match store.list_recent_executions(limit).await {
            Ok(execs) => execs,
            Err(e) => {
                return Json(ApiResponse::error(format!(
                    "Failed to list executions: {}",
                    e
                )));
            }
        }
    };

    // Apply additional filters in memory
    let summaries: Vec<ExecutionSummary> = executions
        .into_iter()
        .filter(|e| {
            query
                .status
                .as_ref()
                .is_none_or(|s| e.status.to_string() == *s)
        })
        .filter(|e| query.from.is_none_or(|from| e.created_at >= from))
        .filter(|e| query.to.is_none_or(|to| e.created_at <= to))
        .map(|e| ExecutionSummary {
            id: e.id,
            channel_type: e.channel_type,
            channel_id: e.channel_id,
            user_id: e.user_id,
            input_text: e.input_text,
            output_text: e.output_text,
            status: e.status.to_string(),
            created_at: e.created_at,
            completed_at: e.completed_at,
        })
        .collect();

    Json(ApiResponse::success(summaries))
}

/// Get execution details by ID (requires authentication)
#[utoipa::path(
    get,
    path = "/api/v1/executions/{id}",
    tag = "executions",
    params(
        ("id" = Uuid, Path, description = "Execution ID")
    ),
    responses(
        (status = 200, description = "Execution details", body = ExecutionDetail),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Execution not found")
    ),
    security(("api_key" = []))
)]
pub async fn get_execution(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Fetch execution
    let execution = match store.get_execution(id).await {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error(format!(
                    "Execution not found: {}",
                    e
                ))),
            )
                .into_response();
        }
    };

    // Fetch associated events
    let events = match store.get_execution_events(id).await {
        Ok(evts) => evts,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(format!(
                    "Failed to load events: {}",
                    e
                ))),
            )
                .into_response();
        }
    };

    let event_summaries: Vec<EventSummary> = events
        .into_iter()
        .map(|ev| EventSummary {
            id: ev.id,
            sequence_num: ev.sequence_num,
            event_type: ev.event_type.as_str().to_string(),
            timestamp: ev.timestamp,
            duration_ms: ev.duration_ms,
        })
        .collect();

    let detail = ExecutionDetail {
        id: execution.id,
        channel_type: execution.channel_type,
        channel_id: execution.channel_id,
        user_id: execution.user_id,
        thread_id: execution.thread_id,
        input_text: execution.input_text,
        output_text: execution.output_text,
        status: execution.status.to_string(),
        created_at: execution.created_at,
        completed_at: execution.completed_at,
        events: event_summaries,
    };

    Json(ApiResponse::success(detail)).into_response()
}

/// Get replay timeline events for an execution (requires authentication)
#[utoipa::path(
    get,
    path = "/api/v1/executions/{id}/replay",
    tag = "executions",
    params(
        ("id" = Uuid, Path, description = "Execution ID")
    ),
    responses(
        (status = 200, description = "Replay timeline events"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Execution not found")
    ),
    security(("api_key" = []))
)]
pub async fn get_replay_events(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Check if execution exists
    if let Err(e) = store.get_execution(id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error(format!(
                "Execution not found: {}",
                e
            ))),
        )
            .into_response();
    }

    let viewer = ExecutionViewer::new((*store).clone());
    match viewer.get_timeline(id).await {
        Ok(timeline) => Json(ApiResponse::success(timeline)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!(
                "Failed to get replay events: {}",
                e
            ))),
        )
            .into_response(),
    }
}

/// Rerun an execution with replay options (requires authentication)
#[utoipa::path(
    post,
    path = "/api/v1/executions/{id}/rerun",
    tag = "executions",
    params(
        ("id" = Uuid, Path, description = "Execution ID")
    ),
    responses(
        (status = 200, description = "Rerun result"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Execution not found")
    ),
    security(("api_key" = []))
)]
pub async fn rerun_execution(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
    Path(id): Path<Uuid>,
    Json(options): Json<ReplayOptions>,
) -> impl IntoResponse {
    let viewer = ExecutionViewer::new((*store).clone());
    match viewer.rerun(id, options).await {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (
                status,
                Json(ApiResponse::<()>::error(format!(
                    "Failed to rerun execution: {}",
                    e
                ))),
            )
                .into_response()
        }
    }
}

/// Get execution statistics for traffic analysis (requires authentication)
#[utoipa::path(
    get,
    path = "/api/v1/executions/stats",
    tag = "executions",
    responses(
        (status = 200, description = "Execution statistics", body = ExecutionStats),
        (status = 401, description = "Unauthorized")
    ),
    security(("api_key" = []))
)]
pub async fn get_execution_stats(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
) -> Json<ApiResponse<ExecutionStats>> {
    // Fetch recent executions (up to 200)
    let executions = match store.list_recent_executions(200).await {
        Ok(execs) => execs,
        Err(e) => {
            return Json(ApiResponse::error(format!("Failed to fetch stats: {}", e)));
        }
    };

    // Group by hour for the last 24 hours
    let now = Utc::now();
    let mut counts = std::collections::HashMap::new();

    // Initialize last 24h buckets
    for i in 0..24 {
        let hour = now - chrono::Duration::hours(i);
        let key = hour.format("%H:00").to_string();
        counts.insert(key, 0);
    }

    // Aggregate
    for exec in executions {
        if exec.created_at > now - chrono::Duration::hours(24) {
            let key = exec.created_at.format("%H:00").to_string();
            // Simplify: just increment the bucket for that hour
            *counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut labels = Vec::new();
    let mut series = Vec::new();

    // Simple approach: Return last 7 hours hourly
    for i in (0..7).rev() {
        let time = now - chrono::Duration::hours(i);
        let key = time.format("%H:00").to_string();
        labels.push(key.clone());
        series.push(*counts.get(&key).unwrap_or(&0) as f64);
    }

    Json(ApiResponse::success(ExecutionStats { labels, series }))
}
