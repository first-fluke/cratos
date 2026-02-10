//! Application-level tools that bridge multiple crates
//!
//! These tools live in the binary crate because they depend on both
//! `cratos-skills` and `cratos-core`, which `cratos-tools` cannot depend on.

pub mod status;

pub use status::StatusTool;
