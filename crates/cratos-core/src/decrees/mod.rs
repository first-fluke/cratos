//! Decrees - Rules System
//!
//! The HOW layer of Olympus OS: Laws, rank system, development rules
//!
//! # Example
//!
//! ```ignore
//! use cratos_core::decrees::DecreeLoader;
//!
//! let loader = DecreeLoader::new();
//! let laws = loader.load_laws()?;
//!
//! println!("Laws: {} articles", laws.articles.len());
//! for article in &laws.articles {
//!     println!("Article {}: {}", article.id, article.title);
//! }
//! ```

mod enforcer;
mod laws;
mod loader;
mod ranks;
mod warfare;

pub use enforcer::{EnforcementAction, EnforcerConfig, LawEnforcer, LawViolation};
pub use laws::{Article, Laws};
pub use loader::{DecreeLoader, ExtendedDecreeResult, ValidationResult};
pub use ranks::{Rank, RankLevel, Ranks};
pub use warfare::{Warfare, WarfareSection};
