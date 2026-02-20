//! Chronicles - Achievement Record System
//!
//! The WHAT layer of Olympus OS: Achievement records and evaluation
//!
//! # Overview
//!
//! Chronicles manages persona activity records, quests, and evaluations.
//!
//! # Example
//!
//! ```rust,ignore
//! use cratos_core::chronicles::{Chronicle, ChronicleStore};
//!
//! let store = ChronicleStore::new();
//! let mut chronicle = Chronicle::new("sindri");
//! chronicle.add_entry("API implementation complete", Some("2"));
//! store.save(&chronicle)?;
//! ```

#![forbid(unsafe_code)]

mod record;
mod store;

pub use record::{Chronicle, ChronicleEntry, ChronicleStatus, Judgment, Quest};
pub use store::ChronicleStore;

#[cfg(test)]
mod tests;

