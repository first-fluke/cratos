use super::types::TelemetryEvent;
use std::sync::atomic::{AtomicU64, Ordering};

/// Aggregated statistics (local only, never sent)
#[derive(Debug, Default)]
pub struct TelemetryStats {
    /// Total commands executed
    pub commands_executed: AtomicU64,
    /// Total successful commands
    pub commands_succeeded: AtomicU64,
    /// Total LLM tokens used
    pub tokens_used: AtomicU64,
    /// Total tools executed
    pub tools_executed: AtomicU64,
    /// Total skills used
    pub skills_used: AtomicU64,
}

impl TelemetryStats {
    /// Get command success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        let total = self.commands_executed.load(Ordering::Relaxed);
        let succeeded = self.commands_succeeded.load(Ordering::Relaxed);

        if total == 0 {
            1.0
        } else {
            succeeded as f64 / total as f64
        }
    }

    /// Update stats based on event
    pub fn update(&self, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::CommandExecuted { success, .. } => {
                self.commands_executed.fetch_add(1, Ordering::Relaxed);
                if *success {
                    self.commands_succeeded.fetch_add(1, Ordering::Relaxed);
                }
            }
            TelemetryEvent::LlmUsed { tokens, .. } => {
                self.tokens_used
                    .fetch_add(u64::from(*tokens), Ordering::Relaxed);
            }
            TelemetryEvent::ToolExecuted { .. } => {
                self.tools_executed.fetch_add(1, Ordering::Relaxed);
            }
            TelemetryEvent::SkillUsed { .. } => {
                self.skills_used.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}
