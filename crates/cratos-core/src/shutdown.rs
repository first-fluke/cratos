//! Graceful Shutdown Manager
//!
//! Provides coordinated shutdown across all components.
//!
//! ## Usage
//!
//! ```ignore
//! let shutdown = ShutdownController::new();
//!
//! // Give tokens to components
//! let token = shutdown.token();
//! component.run(token).await;
//!
//! // Trigger shutdown
//! shutdown.shutdown().await;
//! ```

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Default shutdown timeout in seconds
const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 30;

/// Shutdown phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownPhase {
    /// Normal operation
    Running,
    /// Graceful shutdown initiated - stop accepting new work
    Stopping,
    /// Waiting for active tasks to complete
    Draining,
    /// Force shutdown - abort remaining tasks
    Terminating,
    /// Shutdown complete
    Terminated,
}

impl std::fmt::Display for ShutdownPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Stopping => write!(f, "Stopping"),
            Self::Draining => write!(f, "Draining"),
            Self::Terminating => write!(f, "Terminating"),
            Self::Terminated => write!(f, "Terminated"),
        }
    }
}

/// Shutdown controller for coordinating graceful shutdown
pub struct ShutdownController {
    /// Cancellation token for all components
    cancel_token: CancellationToken,
    /// Broadcast channel for shutdown events
    shutdown_tx: broadcast::Sender<ShutdownPhase>,
    /// Current shutdown phase
    phase: AtomicU32,
    /// Whether shutdown has been initiated
    shutdown_initiated: AtomicBool,
    /// Active task count
    active_tasks: AtomicU32,
    /// Shutdown timeout
    timeout: Duration,
}

impl ShutdownController {
    /// Create a new shutdown controller with default timeout
    #[must_use]
    pub fn new() -> Arc<Self> {
        Self::with_timeout(Duration::from_secs(DEFAULT_SHUTDOWN_TIMEOUT_SECS))
    }

    /// Create a new shutdown controller with custom timeout
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Arc<Self> {
        let (shutdown_tx, _) = broadcast::channel(16);
        Arc::new(Self {
            cancel_token: CancellationToken::new(),
            shutdown_tx,
            phase: AtomicU32::new(ShutdownPhase::Running as u32),
            shutdown_initiated: AtomicBool::new(false),
            active_tasks: AtomicU32::new(0),
            timeout,
        })
    }

    /// Get a cancellation token for a component
    #[must_use]
    pub fn token(&self) -> CancellationToken {
        self.cancel_token.child_token()
    }

    /// Subscribe to shutdown phase changes
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownPhase> {
        self.shutdown_tx.subscribe()
    }

    /// Get current shutdown phase
    #[must_use]
    pub fn phase(&self) -> ShutdownPhase {
        match self.phase.load(Ordering::SeqCst) {
            0 => ShutdownPhase::Running,
            1 => ShutdownPhase::Stopping,
            2 => ShutdownPhase::Draining,
            3 => ShutdownPhase::Terminating,
            _ => ShutdownPhase::Terminated,
        }
    }

    /// Check if shutdown has been initiated
    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_initiated.load(Ordering::SeqCst)
    }

    /// Check if still accepting new work
    #[must_use]
    pub fn is_accepting_work(&self) -> bool {
        self.phase() == ShutdownPhase::Running
    }

    /// Register a new active task
    pub fn register_task(&self) -> TaskGuard<'_> {
        self.active_tasks.fetch_add(1, Ordering::SeqCst);
        TaskGuard {
            controller: self,
            completed: false,
        }
    }

    /// Get the count of active tasks
    #[must_use]
    pub fn active_task_count(&self) -> u32 {
        self.active_tasks.load(Ordering::SeqCst)
    }

    /// Set the shutdown phase
    fn set_phase(&self, phase: ShutdownPhase) {
        self.phase.store(phase as u32, Ordering::SeqCst);
        let _ = self.shutdown_tx.send(phase);
        info!(phase = %phase, "Shutdown phase changed");
    }

    /// Initiate graceful shutdown
    ///
    /// This will:
    /// 1. Stop accepting new work
    /// 2. Cancel all running tasks
    /// 3. Wait for tasks to complete (with timeout)
    /// 4. Force terminate if timeout exceeded
    pub async fn shutdown(self: &Arc<Self>) {
        // Only allow one shutdown
        if self
            .shutdown_initiated
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            debug!("Shutdown already initiated");
            return;
        }

        info!("Initiating graceful shutdown...");

        // Phase 1: Stop accepting new work
        self.set_phase(ShutdownPhase::Stopping);

        // Phase 2: Cancel all tasks
        self.set_phase(ShutdownPhase::Draining);
        self.cancel_token.cancel();

        // Wait for active tasks to complete
        let drain_start = std::time::Instant::now();
        let check_interval = Duration::from_millis(100);

        loop {
            let active = self.active_task_count();
            if active == 0 {
                info!("All tasks completed gracefully");
                break;
            }

            let elapsed = drain_start.elapsed();
            if elapsed >= self.timeout {
                warn!(
                    active_tasks = active,
                    timeout_secs = self.timeout.as_secs(),
                    "Shutdown timeout exceeded, force terminating"
                );
                self.set_phase(ShutdownPhase::Terminating);
                break;
            }

            debug!(
                active_tasks = active,
                elapsed_secs = elapsed.as_secs(),
                "Waiting for tasks to complete..."
            );

            tokio::time::sleep(check_interval).await;
        }

        // Phase 3: Terminated
        self.set_phase(ShutdownPhase::Terminated);
        info!("Graceful shutdown complete");
    }

    /// Trigger immediate shutdown (skip graceful draining)
    pub fn force_shutdown(&self) {
        if self
            .shutdown_initiated
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            warn!("Force shutdown initiated");
            self.set_phase(ShutdownPhase::Terminating);
            self.cancel_token.cancel();
            self.set_phase(ShutdownPhase::Terminated);
        }
    }
}

impl Default for ShutdownController {
    fn default() -> Self {
        Arc::try_unwrap(Self::new()).unwrap_or_else(|arc| (*arc).clone())
    }
}

impl Clone for ShutdownController {
    fn clone(&self) -> Self {
        let (shutdown_tx, _) = broadcast::channel(16);
        Self {
            cancel_token: self.cancel_token.clone(),
            shutdown_tx,
            phase: AtomicU32::new(self.phase.load(Ordering::SeqCst)),
            shutdown_initiated: AtomicBool::new(self.shutdown_initiated.load(Ordering::SeqCst)),
            active_tasks: AtomicU32::new(self.active_tasks.load(Ordering::SeqCst)),
            timeout: self.timeout,
        }
    }
}

/// Guard for tracking active tasks
///
/// Automatically decrements the active task count when dropped.
pub struct TaskGuard<'a> {
    controller: &'a ShutdownController,
    completed: bool,
}

impl<'a> TaskGuard<'a> {
    /// Mark the task as completed (prevents double decrement)
    pub fn complete(mut self) {
        self.completed = true;
        self.controller.active_tasks.fetch_sub(1, Ordering::SeqCst);
    }

    /// Check if shutdown was requested
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.controller.cancel_token.is_cancelled()
    }

    /// Get the cancellation token
    #[must_use]
    pub fn token(&self) -> CancellationToken {
        self.controller.token()
    }
}

impl<'a> Drop for TaskGuard<'a> {
    fn drop(&mut self) {
        if !self.completed {
            self.controller.active_tasks.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
pub async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        }
        _ = terminate => {
            info!("Received SIGTERM signal");
        }
    }
}

/// Create a shutdown signal future that integrates with ShutdownController
pub async fn shutdown_signal_with_controller(controller: Arc<ShutdownController>) {
    wait_for_shutdown_signal().await;
    controller.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_phases() {
        let controller = ShutdownController::new();
        assert_eq!(controller.phase(), ShutdownPhase::Running);
        assert!(controller.is_accepting_work());
        assert!(!controller.is_shutting_down());

        controller.shutdown().await;

        assert_eq!(controller.phase(), ShutdownPhase::Terminated);
        assert!(!controller.is_accepting_work());
        assert!(controller.is_shutting_down());
    }

    #[tokio::test]
    async fn test_task_guard() {
        let controller = ShutdownController::new();

        assert_eq!(controller.active_task_count(), 0);

        {
            let _guard1 = controller.register_task();
            let _guard2 = controller.register_task();
            assert_eq!(controller.active_task_count(), 2);
        }

        // Guards dropped, count should be 0
        assert_eq!(controller.active_task_count(), 0);
    }

    #[tokio::test]
    async fn test_task_guard_complete() {
        let controller = ShutdownController::new();

        let guard = controller.register_task();
        assert_eq!(controller.active_task_count(), 1);

        guard.complete();
        assert_eq!(controller.active_task_count(), 0);
    }

    #[tokio::test]
    async fn test_cancellation_propagation() {
        let controller = ShutdownController::new();
        let token = controller.token();

        assert!(!token.is_cancelled());

        controller.cancel_token.cancel();

        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_double_shutdown() {
        let controller = ShutdownController::new();

        // First shutdown
        let c1 = controller.clone();
        let handle1 = tokio::spawn(async move {
            Arc::new(c1).shutdown().await;
        });

        // Second shutdown (should be no-op)
        let c2 = controller.clone();
        let handle2 = tokio::spawn(async move {
            Arc::new(c2).shutdown().await;
        });

        let _ = tokio::join!(handle1, handle2);

        assert_eq!(controller.phase(), ShutdownPhase::Terminated);
    }

    #[test]
    fn test_force_shutdown() {
        let controller = ShutdownController::new();

        controller.force_shutdown();

        assert_eq!(controller.phase(), ShutdownPhase::Terminated);
        assert!(controller.cancel_token.is_cancelled());
    }
}
