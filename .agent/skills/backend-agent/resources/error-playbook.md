# Backend Agent - Error Playbook

Strategies for resolving common Rust/Axum/SQLx issues.

## 1. SQLx Compile-Time Errors (`query!`)

**Symptom**: `error: database URL must be specified to use sqlx macros` or schema mismatch.

**Solution**:
1. Ensure `.env` contains `DATABASE_URL`.
2. Ensure database is running and reachable.
3. Run `sqlx migrate run` to sync DB schema with migrations.
4. If schema changed, run `cargo sqlx prepare` (if using offline mode).

## 2. Axum Handler Type Mismatch

**Symptom**: `the trait Bound<...> is not implemented for ...`

**Solution**:
1. Check extractor order: `State` must usually come before `Json` or `Path`.
2. Ensure return type implements `IntoResponse`.
3. Verify all extractors implement `FromRequest`.

## 3. Tokio Runtime Panics

**Symptom**: `there is no reactor running, must be called from the context of a Tokio 1.x runtime`

**Solution**:
1. Ensure `#[tokio::main]` or `#[tokio::test]` is present on entry/test function.
2. Don't call async code from blocking contexts (use `tokio::task::block_in_place` only if necessary, or refactor).

## 4. Borrow Checker Issues (Lifecycle)

**Symptom**: `borrowed value does not live long enough` in async block.

**Solution**:
1. Use `Arc<T>` for shared state (like DB pools).
2. Clone `Arc` before moving into async block/closure (`let state = state.clone();`).
3. Use `move` keyword for async blocks: `async move { ... }`.

## 5. Serialization Errors

**Symptom**: `Json rejections` or `Serde error`.

**Solution**:
1. Check `Content-Type` header (must be `application/json`).
2. Verify JSON field names match struct fields (or use `#[serde(rename = "...")]`).
3. Check optional fields (`Option<T>`).
