
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_under_limit() {
        let limiter = RateLimiter::new(RateLimitConfig::new(5, Duration::from_secs(60)));

        for _ in 0..5 {
            let result = limiter.acquire("user1").await;
            assert!(result.allowed);
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_denies_over_limit() {
        let limiter = RateLimiter::new(RateLimitConfig::new(3, Duration::from_secs(60)));

        // Use up the limit
        for _ in 0..3 {
            let result = limiter.acquire("user1").await;
            assert!(result.allowed);
        }

        // Should be denied
        let result = limiter.acquire("user1").await;
        assert!(!result.allowed);
        assert_eq!(result.remaining, 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_separate_keys() {
        let limiter = RateLimiter::new(RateLimitConfig::new(2, Duration::from_secs(60)));

        // User 1 uses their limit
        limiter.acquire("user1").await;
        limiter.acquire("user1").await;
        let result1 = limiter.acquire("user1").await;
        assert!(!result1.allowed);

        // User 2 should still have their limit
        let result2 = limiter.acquire("user2").await;
        assert!(result2.allowed);
    }

    #[tokio::test]
    async fn test_rate_limiter_check_without_record() {
        let limiter = RateLimiter::new(RateLimitConfig::new(5, Duration::from_secs(60)));

        // Check doesn't consume - remaining shows what would be left after this request
        let result1 = limiter.check("user1").await;
        assert!(result1.allowed);
        assert_eq!(result1.remaining, 4); // 5 - 1 = 4 would remain after this request

        let result2 = limiter.check("user1").await;
        assert!(result2.allowed);
        assert_eq!(result2.remaining, 4); // Still 4 since check doesn't record

        // Record consumes
        limiter.record("user1").await;
        let result3 = limiter.check("user1").await;
        assert!(result3.allowed);
        assert_eq!(result3.current, 2); // Will be 2 after this request (1 recorded + 1 new)
        assert_eq!(result3.remaining, 3); // 5 - 2 = 3 would remain
    }

    #[tokio::test]
    async fn test_rate_limiter_usage() {
        let limiter = RateLimiter::new(RateLimitConfig::new(10, Duration::from_secs(60)));

        let (current, max) = limiter.usage("user1").await;
        assert_eq!(current, 0);
        assert_eq!(max, 10);

        limiter.acquire("user1").await;
        limiter.acquire("user1").await;

        let (current, max) = limiter.usage("user1").await;
        assert_eq!(current, 2);
        assert_eq!(max, 10);
    }

    #[tokio::test]
    async fn test_rate_limiter_reset() {
        let limiter = RateLimiter::new(RateLimitConfig::new(2, Duration::from_secs(60)));

        limiter.acquire("user1").await;
        limiter.acquire("user1").await;

        let result = limiter.acquire("user1").await;
        assert!(!result.allowed);

        // Reset
        limiter.reset("user1").await;

        let result = limiter.acquire("user1").await;
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_tiered_rate_limiter() {
        let tiered = TieredRateLimiter::new(
            RateLimitConfig::new(5, Duration::from_secs(60)), // Per user
            RateLimitConfig::new(10, Duration::from_secs(60)), // Global
        );

        // User limit should apply
        for _ in 0..5 {
            let result = tiered.acquire("user1").await;
            assert!(result.allowed);
        }

        let result = tiered.acquire("user1").await;
        assert!(!result.allowed);

        // Different user should still work (global not exhausted)
        let result = tiered.acquire("user2").await;
        assert!(result.allowed);
    }

    #[test]
    fn test_config_builders() {
        let per_sec = RateLimitConfig::per_second(10);
        assert_eq!(per_sec.max_requests, 10);
        assert_eq!(per_sec.window, Duration::from_secs(1));

        let per_min = RateLimitConfig::per_minute(60);
        assert_eq!(per_min.max_requests, 60);
        assert_eq!(per_min.window, Duration::from_secs(60));

        let per_hour = RateLimitConfig::per_hour(1000);
        assert_eq!(per_hour.max_requests, 1000);
        assert_eq!(per_hour.window, Duration::from_secs(3600));
    }
