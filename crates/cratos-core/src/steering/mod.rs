/// Steering context implementation.
pub mod context;
/// Steering handle implementation.
pub mod handle;
/// Steering types.
pub mod types;

pub use context::SteeringContext;
pub use handle::SteerHandle;
pub use types::{SteerDecision, SteerError, SteerMessage, SteerState};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_steer_injection() {
        let mut ctx = SteeringContext::new(Uuid::new_v4());
        let handle = ctx.handle();

        handle
            .inject_message("Change direction".into())
            .await
            .unwrap();
        // Allow some time for channel processing if we were using a real async runtime
        // But since we use try_recv, we might need to be careful.
        // Mpsc channel send IS async, so we awaited it.
        // try_recv should pick it up immediately if it's buffered.

        let decision = ctx.check_before_tool().await.unwrap();
        assert!(matches!(decision, SteerDecision::Continue));
        assert!(matches!(
            *ctx.state.read().await,
            SteerState::Pending(SteerMessage::UserText { .. })
        ));

        let injected = ctx.apply_after_tool().await;
        assert_eq!(injected, Some("Change direction".to_string()));
        assert!(matches!(*ctx.state.read().await, SteerState::Running));
    }

    #[tokio::test]
    async fn test_abort_stops_execution() {
        let mut ctx = SteeringContext::new(Uuid::new_v4());
        ctx.handle()
            .abort(Some("User cancelled".into()))
            .await
            .unwrap();

        let decision = ctx.check_before_tool().await.unwrap();
        assert!(matches!(decision, SteerDecision::Abort(_)));
    }
}
