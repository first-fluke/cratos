use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::error::{Result, TtsError};

#[derive(Debug, Clone, Copy)]
pub enum ElevenLabsTier {
    Free,
    Starter,
    Creator,
    Pro,
    Scale,
}

#[derive(Debug, Default)]
struct UsageStats {
    requests_this_minute: u32,
    characters_this_month: u64,
}

pub struct ElevenLabsRateLimiter {
    requests_per_minute: u32,
    monthly_character_limit: u64,
    current_usage: Arc<RwLock<UsageStats>>,
}

impl ElevenLabsRateLimiter {
    pub fn new(tier: ElevenLabsTier) -> Self {
        let (rpm, chars) = match tier {
            ElevenLabsTier::Free => (100, 10_000),
            ElevenLabsTier::Starter => (500, 30_000),
            ElevenLabsTier::Creator => (1000, 100_000),
            ElevenLabsTier::Pro => (2000, 500_000),
            ElevenLabsTier::Scale => (5000, 2_000_000),
        };

        Self {
            requests_per_minute: rpm,
            monthly_character_limit: chars,
            current_usage: Arc::new(RwLock::new(UsageStats::default())),
        }
    }

    pub async fn check(&self, text_len: usize) -> Result<()> {
        let mut usage = self.current_usage.write().await;

        // Check RPM
        if usage.requests_this_minute >= self.requests_per_minute {
            return Err(TtsError::RateLimitExceeded {
                limit_type: "rpm".into(),
                retry_after: Some(Duration::from_secs(60)),
            });
        }

        // Check Monthly Limit
        if usage.characters_this_month + text_len as u64 > self.monthly_character_limit {
            return Err(TtsError::RateLimitExceeded {
                limit_type: "monthly".into(),
                retry_after: None,
            });
        }

        usage.requests_this_minute += 1;
        usage.characters_this_month += text_len as u64;

        Ok(())
    }

    // Reset RPM counter (should be called every minute ideally, or handle logic differently)
    // For simplicity, we assume external scheduler or just rough estimation here.
    // In a real implementation, we might track timestamps or use a token bucket.
    pub async fn reset_minute_counter(&self) {
        let mut usage = self.current_usage.write().await;
        usage.requests_this_minute = 0;
    }
}
