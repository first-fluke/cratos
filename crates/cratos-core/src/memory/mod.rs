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

mod cache;
mod persistent;
mod redis_store;
mod session;
mod store;
mod working;

pub use cache::MemoryStore;
pub use persistent::{SessionBackend, SessionBackendConfig, SqliteStore};
pub use redis_store::RedisStore;
pub use session::SessionContext;
pub use store::SessionStore;
pub use working::{ToolExecution, WorkingMemory};
