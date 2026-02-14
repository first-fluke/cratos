//! Rate limiting middleware for Axum
//!
//! Wraps the existing `RateLimiter` from cratos-core as an Axum layer.
//! Supports per-IP and per-token dual limiting.

use axum::{
    extract::ConnectInfo,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cratos_core::utils::{RateLimitConfig, RateLimiter};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Layer, Service};
use tracing::warn;

// ============================================================================
// Config
// ============================================================================

/// Rate limit configuration (deserializable from TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitSettings {
    /// Enable rate limiting
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// REST requests per minute per token
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u32,
    /// WS messages per minute per connection
    #[serde(default = "default_ws_rpm")]
    pub ws_messages_per_minute: u32,
    /// Maximum WS message size in bytes
    #[serde(default = "default_max_ws_bytes")]
    pub max_ws_message_bytes: usize,
    /// Global requests per minute (all users combined)
    #[serde(default = "default_global_rpm")]
    pub global_requests_per_minute: u32,
}

fn default_true() -> bool {
    true
}
fn default_rpm() -> u32 {
    60
}
fn default_ws_rpm() -> u32 {
    120
}
fn default_max_ws_bytes() -> usize {
    1_048_576
}
fn default_global_rpm() -> u32 {
    1000
}

impl Default for RateLimitSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: default_rpm(),
            ws_messages_per_minute: default_ws_rpm(),
            max_ws_message_bytes: default_max_ws_bytes(),
            global_requests_per_minute: default_global_rpm(),
        }
    }
}

// ============================================================================
// Rate Limit Error Response
// ============================================================================

#[derive(Debug, Serialize)]
struct RateLimitResponse {
    success: bool,
    error: String,
    code: String,
    retry_after_secs: u64,
}

// ============================================================================
// Rate Limit State (shared across requests)
// ============================================================================

/// Shared rate limiter state
#[derive(Clone)]
pub struct RateLimitState {
    /// Per-key limiter (keyed by IP or token)
    per_key: Arc<RateLimiter>,
    /// Global limiter
    global: Arc<RateLimiter>,
    /// Whether rate limiting is enabled
    enabled: bool,
}

impl RateLimitState {
    /// Create a new rate limit state from settings
    pub fn new(settings: &RateLimitSettings) -> Self {
        let per_key_config = RateLimitConfig::per_minute(settings.requests_per_minute);
        let global_config = RateLimitConfig::per_minute(settings.global_requests_per_minute);

        Self {
            per_key: Arc::new(RateLimiter::new(per_key_config)),
            global: Arc::new(RateLimiter::new(global_config)),
            enabled: settings.enabled,
        }
    }

    /// Check and record a request, returning error if rate limited
    pub async fn check_request(&self, key: &str) -> std::result::Result<(), (u64, u32)> {
        if !self.enabled {
            return Ok(());
        }

        // Check global limit first
        let global_result = self.global.acquire("global").await;
        if !global_result.allowed {
            return Err((global_result.reset_after.as_secs(), global_result.remaining));
        }

        // Check per-key limit
        let key_result = self.per_key.acquire(key).await;
        if !key_result.allowed {
            return Err((key_result.reset_after.as_secs(), key_result.remaining));
        }

        Ok(())
    }

    /// Spawn periodic cleanup task
    pub fn spawn_cleanup(&self) {
        let per_key = self.per_key.clone();
        let global = self.global.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                let _ = per_key.cleanup().await;
                let _ = global.cleanup().await;
            }
        });
    }
}

// ============================================================================
// Axum Layer
// ============================================================================

/// Rate limiting layer for Axum
#[derive(Clone)]
pub struct RateLimitLayer {
    state: RateLimitState,
}

impl RateLimitLayer {
    /// Create a new rate limit layer
    pub fn new(settings: &RateLimitSettings) -> Self {
        Self {
            state: RateLimitState::new(settings),
        }
    }

    /// Get the inner state (for WS message rate limiting)
    pub fn state(&self) -> &RateLimitState {
        &self.state
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            state: self.state.clone(),
        }
    }
}

// ============================================================================
// Axum Service
// ============================================================================

/// Rate limiting service wrapper
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    state: RateLimitState,
}

type BoxFuture<T, E> =
    std::pin::Pin<Box<dyn std::future::Future<Output = std::result::Result<T, E>> + Send>>;

impl<S, B> Service<Request<B>> for RateLimitService<S>
where
    S: Service<Request<B>, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<Response, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> BoxFuture<Response, S::Error> {
        let state = self.state.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Determine rate limit key from IP or token
            let key = extract_rate_limit_key(&req);

            match state.check_request(&key).await {
                Ok(()) => inner.call(req).await,
                Err((retry_after, _remaining)) => {
                    warn!(key = %key, retry_after_secs = retry_after, "Rate limit exceeded");

                    let body = RateLimitResponse {
                        success: false,
                        error: "Rate limit exceeded. Please retry later.".to_string(),
                        code: "RATE_LIMITED".to_string(),
                        retry_after_secs: retry_after,
                    };

                    let response = (
                        StatusCode::TOO_MANY_REQUESTS,
                        [("Retry-After", retry_after.to_string())],
                        Json(body),
                    )
                        .into_response();

                    Ok(response)
                }
            }
        })
    }
}

/// Extract the rate limit key from a request.
/// Uses token hash if authenticated, falls back to IP address.
fn extract_rate_limit_key<B>(req: &Request<B>) -> String {
    // Try token first (for per-token limiting)
    if let Some(auth_header) = req.headers().get("authorization") {
        if let Ok(value) = auth_header.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                // Use first 16 chars of token as key (don't store full token in limiter)
                let prefix: String = token.chars().take(16).collect();
                return format!("token:{}", prefix);
            }
        }
    }

    if let Some(api_key) = req.headers().get("x-api-key") {
        if let Ok(value) = api_key.to_str() {
            let prefix: String = value.chars().take(16).collect();
            return format!("key:{}", prefix);
        }
    }

    // Fall back to IP
    if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return format!("ip:{}", addr.ip());
    }

    // Fallback: use forwarded header
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(value) = forwarded.to_str() {
            if let Some(ip) = value.split(',').next() {
                return format!("ip:{}", ip.trim());
            }
        }
    }

    "ip:unknown".to_string()
}
