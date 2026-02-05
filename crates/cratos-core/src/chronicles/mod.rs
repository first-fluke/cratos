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
mod tests {
    use super::*;

    #[test]
    fn test_chronicle_new() {
        let chronicle = Chronicle::new("test_persona");
        assert_eq!(chronicle.persona_name, "test_persona");
        assert_eq!(chronicle.level, 1);
        assert_eq!(chronicle.status, ChronicleStatus::Active);
    }

    #[test]
    fn test_chronicle_add_entry() {
        let mut chronicle = Chronicle::new("test");
        chronicle.add_entry("Test task completed", Some("2"));

        assert_eq!(chronicle.log.len(), 1);
        assert_eq!(chronicle.log[0].achievement, "Test task completed");
        assert_eq!(chronicle.log[0].law_reference, Some("2".to_string()));
    }
}
