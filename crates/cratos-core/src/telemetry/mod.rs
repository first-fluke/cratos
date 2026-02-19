//! Telemetry - Usage Statistics Collection
//!
//! This module provides opt-out telemetry for Cratos usage statistics.
//! Telemetry is **enabled by default** but can be disabled in settings.

pub mod client;
pub mod config;
pub mod stats;
pub mod types;

pub use client::Telemetry;
pub use config::TelemetryConfig;
pub use stats::TelemetryStats;
pub use types::TelemetryEvent;

// ============================================================================
// Global Instance
// ============================================================================

/// Global telemetry instance
static TELEMETRY: std::sync::OnceLock<Telemetry> = std::sync::OnceLock::new();

/// Get global telemetry instance
pub fn global_telemetry() -> &'static Telemetry {
    TELEMETRY.get_or_init(Telemetry::with_defaults)
}

/// Initialize global telemetry with custom config
pub fn init_telemetry(config: TelemetryConfig) {
    let _ = TELEMETRY.set(Telemetry::new(config));
}

#[cfg(test)]
mod tests;
