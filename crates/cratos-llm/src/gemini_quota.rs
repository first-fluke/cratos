//! Gemini quota poller (DISABLED).
//!
//! The Code Assist `retrieveUserQuota` endpoint (`cloudcode-pa.googleapis.com`)
//! is no longer safe to use — Google permanently bans accounts that access this
//! internal API from third-party tools (since Jan 2026).
//!
//! Quota tracking is now handled by 429/RetryAfter response-based inference in
//! `quota.rs`. This module is retained for backwards compatibility but the
//! poller always returns `None`.

use tokio::sync::watch;
use tracing::{debug, warn};

// ============================================================================
// Bootstrap helper
// ============================================================================

/// Start the Gemini quota poller if an OAuth access token is available.
///
/// Uses Google AI Pro (Gemini CLI) credentials for higher quotas.
/// Calls the Code Assist `retrieveUserQuota` API while mimicking Gemini CLI.
pub async fn start_gemini_quota_poller() -> Option<watch::Sender<bool>> {
    // 1. Try Google AI Pro tokens (Cratos login)
    if let Some(tokens) = crate::cli_auth::read_cratos_google_pro_oauth() {
        if !crate::cli_auth::is_token_expired(tokens.expiry_date) {
            debug!("Starting Gemini quota poller with Google AI Pro credentials");
            let poller = GeminiQuotaPoller::new(tokens.access_token);
            return Some(poller.spawn());
        }
    }

    // 2. Try Gemini CLI tokens (if installed)
    if let Some(creds) = crate::cli_auth::read_gemini_oauth() {
        if !crate::cli_auth::is_token_expired(creds.expiry_date) {
            debug!("Starting Gemini quota poller with Gemini CLI credentials");
            let poller = GeminiQuotaPoller::new(creds.access_token);
            return Some(poller.spawn());
        }
    }

    debug!("Gemini quota poller not started (no valid Google AI Pro / Gemini CLI tokens found)");
    None
}

// ============================================================================
// Quota Poller Implementation
// ============================================================================

use chrono::{DateTime, Utc};
use serde::Deserialize;

const POLL_INTERVAL_SECS: u64 = 60;
const QUOTA_API_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";
const GEMINI_CLI_USER_AGENT: &str = "GeminiCLI/0.27.3 (Cratos-LLM)";

#[derive(Debug, Deserialize)]
struct QuotaResponse {
    #[serde(default)]
    buckets: Vec<QuotaBucket>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct QuotaBucket {
    #[serde(default, rename = "remainingFraction")]
    remaining_fraction: f64,
    #[serde(default, rename = "resetTime")]
    reset_time: Option<String>,
    #[serde(default, rename = "modelId")]
    model_id: Option<String>,
    #[serde(default, rename = "tokenType")]
    token_type: Option<String>,
}

/// Poller for Gemini quota information.
///
/// Periodically calls the Code Assist API to retrieve the user's quota status
/// and updates the global quota tracker.
pub struct GeminiQuotaPoller {
    client: reqwest::Client,
    access_token: String,
}

impl GeminiQuotaPoller {
    /// Create a new Gemini quota poller with the given access token.
    pub fn new(access_token: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent(GEMINI_CLI_USER_AGENT)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            access_token,
        }
    }

    async fn poll_once(&self) -> crate::Result<()> {
        debug!("Polling Gemini quota...");
        let resp = self
            .client
            .post(QUOTA_API_URL)
            .bearer_auth(&self.access_token)
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .map_err(|e| crate::Error::Network(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            
            if status.as_u16() == 401 || status.as_u16() == 403 {
                warn!("Gemini quota API authentication failed ({}). Stopping poller.", status);
                return Err(crate::Error::Api("Unauthorized".to_string()));
            }

            return Err(crate::Error::Api(format!(
                "Gemini quota API returned {}: {}",
                status, body
            )));
        }

        let data: QuotaResponse = resp
            .json()
            .await
            .map_err(|e| crate::Error::InvalidResponse(e.to_string()))?;

        if data.buckets.is_empty() {
            debug!("Gemini quota API returned empty buckets");
            return Ok(());
        }

        let min_fraction = data
            .buckets
            .iter()
            .map(|b| b.remaining_fraction)
            .fold(f64::MAX, f64::min);

        let reset_at = data
            .buckets
            .iter()
            .filter_map(|b| b.reset_time.as_deref())
            .filter_map(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .min();

        let state = crate::quota::QuotaState {
            provider: "gemini".to_string(),
            requests_remaining: None,
            requests_limit: None,
            tokens_remaining: None,
            tokens_limit: None,
            reset_at,
            updated_at: Utc::now(),
            source: crate::quota::QuotaSource::PolledApi,
            remaining_fraction: Some(if min_fraction == f64::MAX {
                1.0
            } else {
                min_fraction.clamp(0.0, 1.0)
            }),
            tier_label: Some("pro".to_string()), // Using Gemini CLI creds implies Pro/High Quota
        };

        crate::quota::global_quota_tracker().update_state(state).await;
        debug!(
            "Gemini quota updated: remaining_fraction={:.2}",
            min_fraction
        );

        Ok(())
    }

    /// Spawn the poller as a background task.
    ///
    /// Returns a shutdown sender that can be used to stop the poller.
    pub fn spawn(self) -> watch::Sender<bool> {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            if let Err(e) = self.poll_once().await {
                warn!("Gemini quota initial poll failed: {}", e);
                // If it's an auth error, don't bother continuing
                if matches!(e, crate::Error::Api(ref m) if m == "Unauthorized") {
                    return;
                }
            }

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)) => {}
                    _ = shutdown_rx.changed() => {
                        debug!("Gemini quota poller shutting down");
                        return;
                    }
                }

                if let Err(e) = self.poll_once().await {
                    warn!("Gemini quota poll failed: {}", e);
                    if matches!(e, crate::Error::Api(ref m) if m == "Unauthorized") {
                        return;
                    }
                }
            }
        });

        shutdown_tx
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    #[test]
    fn test_parse_quota_response() {
        let json = r#"{
            "buckets": [
                {
                    "remainingFraction": 0.85,
                    "resetTime": "2026-02-08T15:30:00Z",
                    "modelId": "gemini-2.5-flash",
                    "tokenType": "INPUT"
                },
                {
                    "remainingFraction": 0.72,
                    "resetTime": "2026-02-08T15:30:00Z",
                    "modelId": "gemini-2.5-flash",
                    "tokenType": "OUTPUT"
                }
            ]
        }"#;

        let resp: QuotaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.buckets.len(), 2);
        assert!((resp.buckets[0].remaining_fraction - 0.85).abs() < 0.001);
        assert_eq!(
            resp.buckets[0].model_id.as_deref(),
            Some("gemini-2.5-flash")
        );
    }

    #[test]
    fn test_reset_time_parsing() {
        let ts = "2026-02-08T15:30:00Z";
        let dt = DateTime::parse_from_rfc3339(ts).unwrap();
        assert_eq!(dt.with_timezone(&Utc).timestamp(), 1770564600);
    }
}
