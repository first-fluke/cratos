//! Gemini quota poller — periodically fetches remaining quota from Google's internal API.
//!
//! Uses the `retrieveUserQuota` endpoint (same as Gemini CLI) to get remaining
//! fraction and reset times per model/bucket.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::sync::watch;
use tracing::{debug, warn};

use crate::quota::{global_quota_tracker, QuotaSource, QuotaState};

/// Polling interval in seconds.
const POLL_INTERVAL_SECS: u64 = 45;

/// Gemini quota API endpoint.
const QUOTA_API_URL: &str =
    "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";

// ============================================================================
// Response types
// ============================================================================

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

// ============================================================================
// Poller
// ============================================================================

/// Polls Gemini's quota API at regular intervals.
pub struct GeminiQuotaPoller {
    client: reqwest::Client,
    access_token: String,
}

impl GeminiQuotaPoller {
    /// Create a new poller with the given access token.
    pub fn new(access_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            access_token,
        }
    }

    /// Perform a single poll and update the global quota tracker.
    async fn poll_once(&self) -> crate::Result<()> {
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

        // Find the minimum remaining_fraction across all buckets (most constrained).
        let min_fraction = data
            .buckets
            .iter()
            .map(|b| b.remaining_fraction)
            .fold(f64::MAX, f64::min);

        // Find earliest reset time.
        let reset_at = data
            .buckets
            .iter()
            .filter_map(|b| b.reset_time.as_deref())
            .filter_map(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .min();

        let state = QuotaState {
            provider: "gemini".to_string(),
            requests_remaining: None,
            requests_limit: None,
            tokens_remaining: None,
            tokens_limit: None,
            reset_at,
            updated_at: Utc::now(),
            source: QuotaSource::PolledApi,
            remaining_fraction: Some(if min_fraction == f64::MAX {
                1.0
            } else {
                min_fraction.clamp(0.0, 1.0)
            }),
            tier_label: Some("free".to_string()),
        };

        global_quota_tracker().update_state(state).await;
        debug!(
            "Gemini quota updated: remaining_fraction={:.2}",
            min_fraction
        );

        Ok(())
    }

    /// Spawn the polling loop. Returns a shutdown sender — drop it or send `true` to stop.
    pub fn spawn(self) -> watch::Sender<bool> {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            // Initial poll immediately.
            if let Err(e) = self.poll_once().await {
                warn!("Gemini quota initial poll failed: {}", e);
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
                }
            }
        });

        shutdown_tx
    }
}

// ============================================================================
// Bootstrap helper
// ============================================================================

/// Start the Gemini quota poller if an OAuth access token is available.
///
/// Returns `Some(shutdown_sender)` if the poller was started, `None` otherwise.
pub async fn start_gemini_quota_poller() -> Option<watch::Sender<bool>> {
    // Try Cratos OAuth tokens first, then Gemini CLI credentials.
    let access_token = if let Some(tokens) = crate::cli_auth::read_cratos_google_oauth() {
        if tokens.access_token.is_empty() {
            return None;
        }
        tokens.access_token
    } else if let Some(creds) = crate::cli_auth::read_gemini_oauth() {
        if creds.access_token.is_empty() {
            return None;
        }
        creds.access_token
    } else {
        debug!("No Gemini OAuth token available — quota poller not started");
        return None;
    };

    debug!("Starting Gemini quota poller");
    let poller = GeminiQuotaPoller::new(access_token);
    Some(poller.spawn())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_min_remaining_fraction() {
        let buckets = vec![
            QuotaBucket {
                remaining_fraction: 0.85,
                reset_time: None,
                model_id: None,
                token_type: None,
            },
            QuotaBucket {
                remaining_fraction: 0.42,
                reset_time: None,
                model_id: None,
                token_type: None,
            },
            QuotaBucket {
                remaining_fraction: 0.72,
                reset_time: None,
                model_id: None,
                token_type: None,
            },
        ];

        let min = buckets
            .iter()
            .map(|b| b.remaining_fraction)
            .fold(f64::MAX, f64::min);
        assert!((min - 0.42).abs() < 0.001);
    }

    #[test]
    fn test_empty_buckets_handled() {
        let json = r#"{ "buckets": [] }"#;
        let resp: QuotaResponse = serde_json::from_str(json).unwrap();
        assert!(resp.buckets.is_empty());
    }

    #[test]
    fn test_reset_time_parsing() {
        let ts = "2026-02-08T15:30:00Z";
        let dt = DateTime::parse_from_rfc3339(ts).unwrap();
        assert_eq!(dt.with_timezone(&Utc).timestamp(), 1770564600);
    }
}
