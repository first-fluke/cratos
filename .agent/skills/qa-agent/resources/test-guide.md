# 테스트 가이드

## 단위 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_function() {
        let result = sync_fn(42);
        assert_eq!(result, 84);
    }

    #[tokio::test]
    async fn test_async_function() {
        let result = async_fn().await;
        assert!(result.is_ok());
    }
}
```

## 통합 테스트

```rust
// tests/integration_test.rs
use cratos_core::*;

#[tokio::test]
async fn test_full_workflow() {
    // 설정
    let app = TestApp::spawn().await;

    // 실행
    let response = app.post("/api/messages")
        .json(&json!({"text": "hello"}))
        .send()
        .await;

    // 검증
    assert_eq!(response.status(), 200);
}
```

## Mock 사용

```rust
use mockall::predicate::*;
use mockall::mock;

mock! {
    pub LlmProvider {}

    #[async_trait]
    impl LlmProvider for LlmProvider {
        async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;
    }
}

#[tokio::test]
async fn test_with_mock() {
    let mut mock = MockLlmProvider::new();
    mock.expect_complete()
        .returning(|_| Ok(CompletionResponse::default()));

    let service = Service::new(Arc::new(mock));
    let result = service.process("test").await;
    assert!(result.is_ok());
}
```

## DB 테스트 (sqlx)

```rust
#[sqlx::test]
async fn test_db_operation(pool: PgPool) {
    let user = create_user(&pool, "test").await.unwrap();
    assert_eq!(user.name, "test");
}
```

## 테스트 커버리지

```bash
# tarpaulin 설치
cargo install cargo-tarpaulin

# 커버리지 실행
cargo tarpaulin --out Html

# 임계값 검사 (70%)
cargo tarpaulin --fail-under 70
```
