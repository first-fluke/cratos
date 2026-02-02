---
name: Rust Patterns for Cratos
description: This skill should be used when implementing Rust patterns specific to Cratos project - async, error handling, testing.
version: 1.0.0
---

# Rust Patterns

Cratos 프로젝트에서 사용하는 Rust 패턴.

## 비동기 패턴

### Tokio spawn + mpsc

```rust
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::channel(100);

tokio::spawn(async move {
    while let Some(msg) = rx.recv().await {
        process(msg).await;
    }
});
```

### Graceful Shutdown

```rust
use tokio::signal;

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to listen for ctrl+c");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
```

## 에러 패턴

### thiserror (라이브러리)

```rust
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Failed to send message: {0}")]
    SendFailed(String),

    #[error("Rate limit exceeded")]
    RateLimited,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

### anyhow (애플리케이션)

```rust
use anyhow::{Context, Result};

pub async fn process() -> Result<()> {
    let data = fetch_data()
        .await
        .context("Failed to fetch data")?;

    Ok(())
}
```

## 테스트 패턴

### Async Test

```rust
#[tokio::test]
async fn test_async() {
    let result = async_fn().await;
    assert!(result.is_ok());
}
```

### Mock (mockall)

```rust
use mockall::predicate::*;
use mockall::mock;

mock! {
    pub Service {}
    #[async_trait]
    impl ServiceTrait for Service {
        async fn call(&self, input: &str) -> Result<String>;
    }
}
```
