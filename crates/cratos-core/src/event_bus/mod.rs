//! EventBus - broadcast-based event system for real-time orchestrator events.
//!
//! Publishes events during execution so that WebSocket clients, REST SSE endpoints,
//! and internal subscribers can receive real-time updates.

/// Core event bus implementation (broadcast channel).
pub mod bus;
/// Event type definitions for orchestrator lifecycle.
pub mod types;

pub use bus::EventBus;
pub use types::OrchestratorEvent;

#[cfg(test)]
mod tests;
