//! Memory - Session and context management
//!
//! This module provides session and memory management for the orchestrator:
//! - Session context (conversation history)
//! - Working memory (temporary state during execution)
//! - SQLite store (default, production-ready)
//! - Redis store (optional, for high-scale scenarios)
//! - In-memory store (development/testing)
//!
//! ## Session Backend Selection
//!
//! By default, Cratos uses SQLite for session storage, which:
//! - Requires no external services
//! - Persists data across restarts
//! - Works out of the box
//!
//! Redis is available for high-scale deployments but is entirely optional.

mod session;
mod sqlite_store;
mod store;
mod working;

pub use session::SessionContext;
pub use sqlite_store::{SessionBackend, SessionBackendConfig, SqliteStore};
pub use store::{MemoryStore, RedisStore, SessionStore};
pub use working::{ToolExecution, WorkingMemory};
