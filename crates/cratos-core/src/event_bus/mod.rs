//! EventBus - broadcast-based event system for real-time orchestrator events.
//!
//! Publishes events during execution so that WebSocket clients, REST SSE endpoints,
//! and internal subscribers can receive real-time updates.

pub mod bus;
pub mod types;

pub use bus::EventBus;
pub use types::OrchestratorEvent;

#[cfg(test)]
mod tests;
