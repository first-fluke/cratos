//! Rate limiter for bash commands

use std::time::Instant;

/// Simple rate limiter that tracks command timestamps
pub(crate) struct RateLimiter {
    timestamps: Vec<Instant>,
    max_per_minute: u32,
}

impl RateLimiter {
    pub fn new(max_per_minute: u32) -> Self {
        Self {
            timestamps: Vec::new(),
            max_per_minute,
        }
    }

    /// Check if a command can be executed (returns false if rate limited)
    pub fn check(&mut self) -> bool {
        let now = Instant::now();
        let one_minute_ago = now - std::time::Duration::from_secs(60);
        self.timestamps.retain(|t| *t > one_minute_ago);
        if self.timestamps.len() >= self.max_per_minute as usize {
            return false;
        }
        self.timestamps.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit() {
        let mut limiter = RateLimiter::new(3);
        assert!(limiter.check()); // 1
        assert!(limiter.check()); // 2
        assert!(limiter.check()); // 3
        assert!(!limiter.check()); // 4 → blocked
    }
}
