//! EventBus - broadcast-based event system for real-time orchestrator events.
//!
//! Publishes events during execution so that WebSocket clients, REST SSE endpoints,
//! and internal subscribers can receive real-time updates.

use serde::Serialize;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Events emitted during orchestrator execution.
///
/// **Security note**: These events intentionally exclude sensitive data (API keys,
/// full tool outputs, etc.). Detailed data should be fetched via authenticated
/// REST endpoints using the `execution_id`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorEvent {
    /// Execution has started
    ExecutionStarted {
        /// Unique execution identifier
        execution_id: Uuid,
        /// Session key for this execution
        session_key: String,
    },
    /// LLM planning step started
    PlanningStarted {
        /// Execution identifier
        execution_id: Uuid,
        /// Current iteration number
        iteration: usize,
    },
    /// Streaming text delta from LLM
    ChatDelta {
        /// Execution identifier
        execution_id: Uuid,
        /// Text chunk
        delta: String,
        /// Whether this is the final chunk
        is_final: bool,
    },
    /// Tool execution started
    ToolStarted {
        /// Execution identifier
        execution_id: Uuid,
        /// Name of the tool being executed
        tool_name: String,
        /// Tool call ID from the LLM
        tool_call_id: String,
    },
    /// Tool execution completed
    ToolCompleted {
        /// Execution identifier
        execution_id: Uuid,
        /// Tool call ID
        tool_call_id: String,
        /// Whether the tool succeeded
        success: bool,
        /// Execution duration in milliseconds
        duration_ms: u64,
    },
    /// User approval is required
    ApprovalRequired {
        /// Execution identifier
        execution_id: Uuid,
        /// Approval request ID
        request_id: Uuid,
    },
    /// Execution completed successfully
    ExecutionCompleted {
        /// Execution identifier
        execution_id: Uuid,
    },
    /// Execution failed
    ExecutionFailed {
        /// Execution identifier
        execution_id: Uuid,
        /// Error description (sanitized, no sensitive data)
        error: String,
    },
    /// Execution was cancelled
    ExecutionCancelled {
        /// Execution identifier
        execution_id: Uuid,
    },
    /// A2A message sent between agents
    A2aMessageSent {
        /// Session identifier
        session_id: String,
        /// Sending agent ID
        from_agent: String,
        /// Receiving agent ID
        to_agent: String,
        /// Message ID
        message_id: Uuid,
    },
}

impl OrchestratorEvent {
    /// Get the execution_id from any event variant.
    #[must_use]
    pub fn execution_id(&self) -> Uuid {
        match self {
            Self::ExecutionStarted { execution_id, .. }
            | Self::PlanningStarted { execution_id, .. }
            | Self::ChatDelta { execution_id, .. }
            | Self::ToolStarted { execution_id, .. }
            | Self::ToolCompleted { execution_id, .. }
            | Self::ApprovalRequired { execution_id, .. }
            | Self::ExecutionCompleted { execution_id }
            | Self::ExecutionFailed { execution_id, .. }
            | Self::ExecutionCancelled { execution_id } => *execution_id,
            Self::A2aMessageSent { message_id, .. } => *message_id,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let exec_id = Uuid::new_v4();
        bus.publish(OrchestratorEvent::ExecutionStarted {
            execution_id: exec_id,
            session_key: "test:1:1".to_string(),
        });

        let event = rx.recv().await.unwrap();
        assert_eq!(event.execution_id(), exec_id);
        match event {
            OrchestratorEvent::ExecutionStarted { session_key, .. } => {
                assert_eq!(session_key, "test:1:1");
            }
            _ => panic!("unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        let exec_id = Uuid::new_v4();
        let count = bus.publish(OrchestratorEvent::ExecutionCompleted {
            execution_id: exec_id,
        });
        assert_eq!(count, 2);

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert_eq!(e1.execution_id(), exec_id);
        assert_eq!(e2.execution_id(), exec_id);
    }

    #[test]
    fn test_publish_no_subscribers() {
        let bus = EventBus::new(16);
        // No subscribers — should not panic
        let count = bus.publish(OrchestratorEvent::ExecutionCancelled {
            execution_id: Uuid::new_v4(),
        });
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_event_ordering() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let exec_id = Uuid::new_v4();
        bus.publish(OrchestratorEvent::ExecutionStarted {
            execution_id: exec_id,
            session_key: "s".to_string(),
        });
        bus.publish(OrchestratorEvent::PlanningStarted {
            execution_id: exec_id,
            iteration: 1,
        });
        bus.publish(OrchestratorEvent::ExecutionCompleted {
            execution_id: exec_id,
        });

        // Events arrive in order
        match rx.recv().await.unwrap() {
            OrchestratorEvent::ExecutionStarted { .. } => {}
            other => panic!("expected ExecutionStarted, got: {:?}", other),
        }
        match rx.recv().await.unwrap() {
            OrchestratorEvent::PlanningStarted { iteration, .. } => {
                assert_eq!(iteration, 1);
            }
            other => panic!("expected PlanningStarted, got: {:?}", other),
        }
        match rx.recv().await.unwrap() {
            OrchestratorEvent::ExecutionCompleted { .. } => {}
            other => panic!("expected ExecutionCompleted, got: {:?}", other),
        }
    }

    #[test]
    fn test_event_serialization() {
        let event = OrchestratorEvent::ToolStarted {
            execution_id: Uuid::nil(),
            tool_name: "file_read".to_string(),
            tool_call_id: "call_1".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"tool_started\""));
        assert!(json.contains("\"tool_name\":\"file_read\""));
    }

    #[test]
    fn test_execution_id_extraction() {
        let id = Uuid::new_v4();
        let events = vec![
            OrchestratorEvent::ExecutionStarted {
                execution_id: id,
                session_key: "s".to_string(),
            },
            OrchestratorEvent::PlanningStarted {
                execution_id: id,
                iteration: 1,
            },
            OrchestratorEvent::ChatDelta {
                execution_id: id,
                delta: "hi".to_string(),
                is_final: false,
            },
            OrchestratorEvent::ToolStarted {
                execution_id: id,
                tool_name: "t".to_string(),
                tool_call_id: "c".to_string(),
            },
            OrchestratorEvent::ToolCompleted {
                execution_id: id,
                tool_call_id: "c".to_string(),
                success: true,
                duration_ms: 100,
            },
            OrchestratorEvent::ApprovalRequired {
                execution_id: id,
                request_id: Uuid::new_v4(),
            },
            OrchestratorEvent::ExecutionCompleted { execution_id: id },
            OrchestratorEvent::ExecutionFailed {
                execution_id: id,
                error: "err".to_string(),
            },
            OrchestratorEvent::ExecutionCancelled { execution_id: id },
        ];

        for event in events {
            assert_eq!(event.execution_id(), id);
        }

        // A2A event uses message_id instead of execution_id
        let msg_id = Uuid::new_v4();
        let a2a_event = OrchestratorEvent::A2aMessageSent {
            session_id: "s1".to_string(),
            from_agent: "a".to_string(),
            to_agent: "b".to_string(),
            message_id: msg_id,
        };
        assert_eq!(a2a_event.execution_id(), msg_id);
    }

    #[test]
    fn test_default_capacity() {
        let bus = EventBus::default();
        assert_eq!(bus.subscriber_count(), 0);
    }
}
