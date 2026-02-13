//! ACP (Agent Client Protocol) bridge module.
//!
//! Provides stdin/stdout JSON-lines ↔ Gateway WS protocol bridging
//! for IDE integration (VS Code, Zed, etc.).
//!
//! Usage: `cratos acp [--token <token>]`

pub mod bridge;
pub mod mcp_compat;
pub mod protocol;
