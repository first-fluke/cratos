//! Quota & Rate Limit API endpoint
//!
//! GET /api/v1/quota — returns per-provider rate limit status and cost summary.

use axum::{routing::get, Json, Router};
use cratos_llm::{global_quota_tracker, global_tracker};
use serde::Serialize;

/// Response for GET /api/v1/quota
#[derive(Debug, Serialize)]
pub struct QuotaResponse {
    pub providers: Vec<ProviderQuota>,
    pub today: TodaySummary,
}

/// Rate limit state for a single provider.
#[derive(Debug, Serialize)]
pub struct ProviderQuota {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_source: Option<String>,
    pub requests: QuotaNumbers,
    pub tokens: QuotaNumbers,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<String>,
    pub reset_in_seconds: i64,
    pub updated_at: String,
    pub warning: bool,
}

/// Numeric quota fields.
#[derive(Debug, Serialize)]
pub struct QuotaNumbers {
    pub remaining: Option<u64>,
    pub limit: Option<u64>,
    pub usage_pct: Option<f64>,
}

/// Today's cost/token summary.
#[derive(Debug, Serialize)]
pub struct TodaySummary {
    pub total_cost_usd: f64,
    pub total_tokens: u64,
}

/// GET /api/v1/quota handler.
async fn get_quota() -> Json<QuotaResponse> {
    let tracker = global_quota_tracker();
    let cost_tracker = global_tracker();

    let states = tracker.get_all_states().await;
    let report = cost_tracker.generate_report(None).await;

    let providers = states
        .iter()
        .map(|s| {
            let reset_in = s
                .reset_at
                .map(|r| (r - chrono::Utc::now()).num_seconds().max(0))
                .unwrap_or(0);

            ProviderQuota {
                name: s.provider.clone(),
                auth_source: cratos_llm::cli_auth::get_auth_source(&s.provider)
                    .filter(|src| *src != cratos_llm::cli_auth::AuthSource::ApiKey)
                    .map(|src| src.to_string()),
                requests: QuotaNumbers {
                    remaining: s.requests_remaining,
                    limit: s.requests_limit,
                    usage_pct: s.requests_usage_pct(),
                },
                tokens: QuotaNumbers {
                    remaining: s.tokens_remaining,
                    limit: s.tokens_limit,
                    usage_pct: s.tokens_usage_pct(),
                },
                reset_at: s.reset_at.map(|r| r.to_rfc3339()),
                reset_in_seconds: reset_in,
                updated_at: s.updated_at.to_rfc3339(),
                warning: s.is_near_limit(20.0),
            }
        })
        .collect();

    let total_tokens = report.stats.total_input_tokens + report.stats.total_output_tokens;

    Json(QuotaResponse {
        providers,
        today: TodaySummary {
            total_cost_usd: report.stats.total_cost,
            total_tokens,
        },
    })
}

/// Create the quota routes.
pub fn quota_routes() -> Router {
    Router::new().route("/api/v1/quota", get(get_quota))
}
