//! Health check endpoints with component-level diagnostics.
//!
//! Provides:
//! - `/health` — simple "healthy" + version (for load balancers)
//! - `/health/detailed` — per-component status (database, redis, llm, scheduler, event_bus)

use axum::extract::Extension;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use cratos_core::event_bus::EventBus;
use cratos_core::orchestrator::Orchestrator;
use cratos_core::scheduler::SchedulerEngine;
use serde::Serialize;
use std::sync::Arc;

use crate::middleware::auth::RequireAuthStrict;

/// Simple health response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// Detailed health response with per-component checks
#[derive(Debug, Serialize)]
pub struct DetailedHealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub checks: HealthChecks,
}

/// All component health checks
#[derive(Debug, Serialize)]
pub struct HealthChecks {
    pub database: ComponentHealth,
    pub redis: ComponentHealth,
    pub llm: ComponentHealth,
    pub scheduler: ComponentHealth,
    pub event_bus: ComponentHealth,
}

/// Individual component health status
#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ComponentHealth {
    fn healthy(latency_ms: u64) -> Self {
        Self {
            status: "healthy",
            latency_ms: Some(latency_ms),
            error: None,
            details: None,
        }
    }

    fn healthy_with_details(latency_ms: u64, details: serde_json::Value) -> Self {
        Self {
            status: "healthy",
            latency_ms: Some(latency_ms),
            error: None,
            details: Some(details),
        }
    }

    fn unhealthy(error: String) -> Self {
        Self {
            status: "unhealthy",
            latency_ms: None,
            error: Some(error),
            details: None,
        }
    }
}

/// Simple health check (for load balancers)
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Detailed health check with all component statuses (requires strict authentication — never bypassed)
async fn detailed_health_check(
    RequireAuthStrict(_auth): RequireAuthStrict,
    redis_url: Extension<String>,
    orchestrator: Extension<Arc<Orchestrator>>,
    event_bus: Extension<Arc<EventBus>>,
    scheduler: Option<Extension<Arc<SchedulerEngine>>>,
) -> Json<DetailedHealthResponse> {
    // Database check (SQLite — file-based, always available)
    let db_health = ComponentHealth::healthy(0);

    // Redis check
    let redis_health = check_redis(redis_url.as_str()).await;

    // LLM provider check (lightweight — just verify provider name + available models)
    let llm_health = check_llm(&orchestrator);

    // Scheduler check
    let scheduler_health = if let Some(Extension(sched)) = scheduler {
        check_scheduler(&sched).await
    } else {
        ComponentHealth {
            status: "disabled",
            latency_ms: None,
            error: None,
            details: None,
        }
    };

    // EventBus check
    let event_bus_health = check_event_bus(&event_bus);

    let components = [
        db_health.status,
        redis_health.status,
        llm_health.status,
        scheduler_health.status,
        event_bus_health.status,
    ];

    let healthy_count = components.iter().filter(|s| **s == "healthy").count();
    let unhealthy_count = components.iter().filter(|s| **s == "unhealthy").count();

    let overall_status = if unhealthy_count == 0 {
        "healthy"
    } else if healthy_count > 0 {
        "degraded"
    } else {
        "unhealthy"
    };

    Json(DetailedHealthResponse {
        status: overall_status,
        version: env!("CARGO_PKG_VERSION"),
        checks: HealthChecks {
            database: db_health,
            redis: redis_health,
            llm: llm_health,
            scheduler: scheduler_health,
            event_bus: event_bus_health,
        },
    })
}

/// Check Redis connectivity
async fn check_redis(redis_url: &str) -> ComponentHealth {
    let start = std::time::Instant::now();
    match redis::Client::open(redis_url) {
        Ok(client) => match client.get_multiplexed_async_connection().await {
            Ok(mut conn) => match redis::cmd("PING").query_async::<String>(&mut conn).await {
                Ok(_) => ComponentHealth::healthy(start.elapsed().as_millis() as u64),
                Err(e) => ComponentHealth::unhealthy(e.to_string()),
            },
            Err(e) => ComponentHealth::unhealthy(e.to_string()),
        },
        Err(e) => ComponentHealth::unhealthy(e.to_string()),
    }
}

/// Check LLM provider availability (lightweight — no API call)
fn check_llm(orchestrator: &Orchestrator) -> ComponentHealth {
    let provider_name = orchestrator.provider_name();
    let models = orchestrator.available_models();
    ComponentHealth::healthy_with_details(
        0,
        serde_json::json!({
            "provider": provider_name,
            "models_available": models.len(),
        }),
    )
}

/// Check scheduler status
async fn check_scheduler(engine: &SchedulerEngine) -> ComponentHealth {
    let start = std::time::Instant::now();
    let running = engine.running_count().await;
    let latency = start.elapsed().as_millis() as u64;
    ComponentHealth::healthy_with_details(
        latency,
        serde_json::json!({
            "running_tasks": running,
        }),
    )
}

/// Check EventBus status
fn check_event_bus(bus: &EventBus) -> ComponentHealth {
    let subscribers = bus.subscriber_count();
    ComponentHealth::healthy_with_details(
        0,
        serde_json::json!({
            "subscriber_count": subscribers,
        }),
    )
}

/// Prometheus metrics endpoint (requires strict authentication — never bypassed)
async fn metrics_endpoint(RequireAuthStrict(_auth): RequireAuthStrict) -> String {
    cratos_core::metrics_global::export_prometheus()
}

/// Create health routes
pub fn health_routes() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/health/detailed", get(detailed_health_check))
        .route("/metrics", get(metrics_endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_health_healthy() {
        let h = ComponentHealth::healthy(42);
        assert_eq!(h.status, "healthy");
        assert_eq!(h.latency_ms, Some(42));
        assert!(h.error.is_none());
    }

    #[test]
    fn test_component_health_unhealthy() {
        let h = ComponentHealth::unhealthy("connection refused".to_string());
        assert_eq!(h.status, "unhealthy");
        assert!(h.latency_ms.is_none());
        assert_eq!(h.error.as_deref(), Some("connection refused"));
    }

    #[test]
    fn test_component_health_with_details() {
        let h = ComponentHealth::healthy_with_details(
            10,
            serde_json::json!({"provider": "gemini"}),
        );
        assert_eq!(h.status, "healthy");
        assert!(h.details.is_some());
    }

    #[test]
    fn test_health_response_serialization() {
        let resp = HealthResponse {
            status: "healthy",
            version: "0.1.0",
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("0.1.0"));
    }
}
