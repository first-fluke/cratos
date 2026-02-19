//! WhatsApp channels

pub mod bridge;
pub mod business;

pub use bridge::{WhatsAppAdapter, WhatsAppConfig};
pub use business::{WhatsAppBusinessAdapter, WhatsAppBusinessConfig};
