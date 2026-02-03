# Graceful Shutdown Guide

A guide to Cratos's safe shutdown mechanism.

## Overview

Graceful Shutdown ensures that when the system receives a termination signal:
- ❌ Immediate forced shutdown (risk of data loss)
- ✅ Complete ongoing work → Save state → Clean up connections → Safe exit

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    ShutdownController                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ CancellToken│  │ Phase State │  │ Active Task Counter │ │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
└─────────┼────────────────┼─────────────────────┼────────────┘
          │                │                     │
          ▼                ▼                     ▼
    ┌──────────┐    ┌──────────┐          ┌──────────┐
    │ HTTP     │    │ Telegram │    ...   │ Agent    │
    │ Server   │    │ Adapter  │          │ Tasks    │
    └──────────┘    └──────────┘          └──────────┘
```

## Shutdown Phases

| Phase | Description | Action |
|-------|-------------|--------|
| **Running** | Normal operation | Process all requests |
| **Stopping** | Shutdown initiated | Reject new requests |
| **Draining** | Cleaning up | Cancel and wait for running tasks |
| **Terminating** | Force shutdown | Abort remaining tasks after timeout |
| **Terminated** | Complete | All resources released |

```
User: Ctrl+C / SIGTERM
           │
           ▼
    ┌──────────────┐
    │   Running    │
    └──────┬───────┘
           │ shutdown() called
           ▼
    ┌──────────────┐
    │   Stopping   │  ← Reject new work
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │   Draining   │  ← Cancel tasks, wait for completion
    └──────┬───────┘
           │
           ├─── All tasks completed ───┐
           │                           │
           │ Timeout (30s)             │
           ▼                           │
    ┌──────────────┐                   │
    │ Terminating  │  ← Force abort    │
    └──────┬───────┘                   │
           │                           │
           ▼                           ▼
    ┌──────────────────────────────────┐
    │           Terminated             │
    └──────────────────────────────────┘
```

## Usage

### Basic Usage

```rust
use cratos_core::{ShutdownController, shutdown_signal_with_controller};

#[tokio::main]
async fn main() {
    // 1. Create controller
    let shutdown = ShutdownController::new();

    // 2. Pass token to components
    let token = shutdown.token();

    tokio::spawn(async move {
        my_component(token).await;
    });

    // 3. Run server (wait for Ctrl+C)
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal_with_controller(shutdown))
        .await;
}
```

### Handling Cancellation in Components

```rust
use tokio_util::sync::CancellationToken;

async fn my_component(cancel_token: CancellationToken) {
    loop {
        tokio::select! {
            // Normal work
            result = do_work() => {
                handle_result(result);
            }
            // Cancellation signal
            _ = cancel_token.cancelled() => {
                info!("Shutdown signal received, cleaning up...");
                cleanup().await;
                return;
            }
        }
    }
}
```

### Task Tracking with TaskGuard

```rust
async fn process_request(
    shutdown: &ShutdownController,
    request: Request,
) -> Result<Response> {
    // Check if accepting new work
    if !shutdown.is_accepting_work() {
        return Err(Error::ServiceUnavailable);
    }

    // Register task (auto-counted)
    let _guard = shutdown.register_task();

    // Do work
    let result = handle_request(request).await?;

    Ok(result)
    // _guard drop automatically decrements count
}
```

### Manual Task Completion

```rust
let guard = shutdown.register_task();

// Do work
do_work().await;

// Explicit completion (call before drop)
guard.complete();
```

## API Reference

### ShutdownController

```rust
impl ShutdownController {
    /// Create new controller (default timeout: 30s)
    pub fn new() -> Arc<Self>;

    /// Create with custom timeout
    pub fn with_timeout(timeout: Duration) -> Arc<Self>;

    /// Get child CancellationToken
    pub fn token(&self) -> CancellationToken;

    /// Subscribe to phase changes
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownPhase>;

    /// Get current phase
    pub fn phase(&self) -> ShutdownPhase;

    /// Check if shutdown in progress
    pub fn is_shutting_down(&self) -> bool;

    /// Check if accepting new work
    pub fn is_accepting_work(&self) -> bool;

    /// Register a task (returns TaskGuard)
    pub fn register_task(&self) -> TaskGuard<'_>;

    /// Get active task count
    pub fn active_task_count(&self) -> u32;

    /// Initiate graceful shutdown
    pub async fn shutdown(self: &Arc<Self>);

    /// Immediate force shutdown
    pub fn force_shutdown(&self);
}
```

### TaskGuard

```rust
impl TaskGuard<'_> {
    /// Mark task as completed
    pub fn complete(self);

    /// Check if cancelled
    pub fn is_cancelled(&self) -> bool;

    /// Get CancellationToken
    pub fn token(&self) -> CancellationToken;
}
// Automatically decrements active_tasks on Drop
```

### Helper Functions

```rust
/// Wait for Ctrl+C / SIGTERM
pub async fn wait_for_shutdown_signal();

/// Shutdown signal integrated with ShutdownController
pub async fn shutdown_signal_with_controller(controller: Arc<ShutdownController>);
```

## Configuration

### Timeout Settings

```rust
use std::time::Duration;

// 60 second timeout
let shutdown = ShutdownController::with_timeout(
    Duration::from_secs(60)
);
```

### Monitoring Phase Changes

```rust
let mut rx = shutdown.subscribe();

tokio::spawn(async move {
    while let Ok(phase) = rx.recv().await {
        match phase {
            ShutdownPhase::Stopping => {
                info!("Started rejecting new requests");
            }
            ShutdownPhase::Draining => {
                info!("Cleaning up running tasks...");
            }
            ShutdownPhase::Terminated => {
                info!("Shutdown complete");
                break;
            }
            _ => {}
        }
    }
});
```

## Channel Adapter Integration

### Telegram Example

```rust
let telegram_shutdown = shutdown_controller.token();

tokio::spawn(async move {
    tokio::select! {
        result = telegram_adapter.run(orchestrator) => {
            if let Err(e) = result {
                error!("Telegram error: {}", e);
            }
        }
        _ = telegram_shutdown.cancelled() => {
            info!("Telegram adapter shutting down...");
            // Cleanup logic
        }
    }
});
```

### Slack Example

```rust
let slack_shutdown = shutdown_controller.token();

tokio::spawn(async move {
    let mut socket = connect_slack().await?;

    loop {
        tokio::select! {
            msg = socket.recv() => {
                handle_message(msg).await;
            }
            _ = slack_shutdown.cancelled() => {
                socket.close().await;
                info!("Slack adapter closed");
                break;
            }
        }
    }
});
```

## Best Practices

### 1. Always Use select!

```rust
// ✅ Good
tokio::select! {
    result = long_running_task() => { /* ... */ }
    _ = cancel_token.cancelled() => { return; }
}

// ❌ Bad - Cannot be cancelled
let result = long_running_task().await;
```

### 2. Implement Cleanup Logic

```rust
async fn my_service(cancel: CancellationToken) {
    let resource = acquire_resource().await;

    let result = tokio::select! {
        r = do_work(&resource) => r,
        _ = cancel.cancelled() => {
            // Must cleanup!
            release_resource(resource).await;
            return;
        }
    };

    release_resource(resource).await;
}
```

### 3. Reject New Work During Shutdown

```rust
async fn handle_request(shutdown: &ShutdownController) -> Result<()> {
    if !shutdown.is_accepting_work() {
        return Err(Error::ServiceUnavailable);
    }

    let _guard = shutdown.register_task();
    // ...
}
```

### 4. Set Appropriate Timeouts

```rust
// Short task service
let shutdown = ShutdownController::with_timeout(Duration::from_secs(10));

// Long task service (AI processing, etc.)
let shutdown = ShutdownController::with_timeout(Duration::from_secs(60));
```

## Troubleshooting

### Shutdown Not Completing

1. **Cause**: Task not checking `cancel_token.cancelled()`
2. **Solution**: Add `tokio::select!` to all long-running tasks

### Timeout Occurred

1. **Cause**: Cleanup takes longer than timeout
2. **Solution**:
   - Increase timeout
   - Optimize cleanup logic
   - Allow force shutdown

### Log Analysis

```
INFO  Initiating graceful shutdown...
INFO  Shutdown phase changed: Stopping
INFO  Shutdown phase changed: Draining
DEBUG Waiting for tasks to complete... active_tasks=3 elapsed_secs=0
DEBUG Waiting for tasks to complete... active_tasks=1 elapsed_secs=5
INFO  All tasks completed gracefully
INFO  Shutdown phase changed: Terminated
INFO  Graceful shutdown complete
```

## Related Documentation

- [CancellationToken](./CANCELLATION_TOKEN.md)
- [Token Budget](./TOKEN_BUDGET.md)
- [Agent Orchestrator](./ORCHESTRATOR.md)
