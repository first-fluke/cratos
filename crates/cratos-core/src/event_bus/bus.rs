use super::types::OrchestratorEvent;
use tokio::sync::broadcast;

/// Broadcast-based event bus for real-time orchestrator events.
///
/// Uses `tokio::broadcast` so multiple subscribers can receive the same events.
/// Slow subscribers will miss events (lagged) rather than blocking the publisher.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<OrchestratorEvent>,
}

impl EventBus {
    /// Create a new EventBus with the given channel capacity.
    ///
    /// Capacity determines how many events can be buffered before slow
    /// subscribers start missing events. 256 is a reasonable default.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to events. Returns a receiver that will get all future events.
    ///
    /// Each subscriber gets an independent copy of every published event.
    /// If a subscriber falls behind by more than `capacity` events, it will
    /// receive a `RecvError::Lagged` on next recv.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<OrchestratorEvent> {
        self.sender.subscribe()
    }

    /// Publish an event to all active subscribers.
    ///
    /// Returns the number of subscribers that received the event.
    /// If there are no subscribers, the event is silently dropped.
    pub fn publish(&self, event: OrchestratorEvent) -> usize {
        // send() returns Err if there are no receivers, which is fine
        self.sender.send(event).unwrap_or(0)
    }

    /// Get the current number of active subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}
