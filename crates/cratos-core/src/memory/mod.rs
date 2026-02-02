//! Memory - Session and context management
//!
//! This module provides session and memory management for the orchestrator:
//! - Session context (conversation history)
//! - Working memory (temporary state during execution)
//! - In-memory store (development/testing)
//! - Redis store (production)

mod session;
mod store;
mod working;

pub use session::SessionContext;
pub use store::{MemoryStore, RedisStore, SessionStore};
pub use working::{ToolExecution, WorkingMemory};
