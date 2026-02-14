# Execution Protocol

Follow these steps to implement new backend features:

## 1. Environment Setup

- Ensure you are in the project root.
- Check database connection: `cargo sqlx database show` (if configured) or ensure `.env` is set up.

## 2. Implementation

1. **Create/Update Models**:
    - Define structs in `crates/[crate_name]/src/[module]/models.rs` or `types.rs`.
    - overload `serde::Serialize`, `serde::Deserialize`, `sqlx::FromRow`.

2. **Create Repository**:
    - implement `Repository` struct in `repository.rs`.
    - methods dealing with `sqlx::Pool<Sqlite>` (or Postgres).
    - use `sqlx::query_as!` macros for compile-time checked queries.

3. **Create Service**:
    - implement `Service` struct in `service.rs`.
    - include business logic.
    - dependency injection via struct fields (e.g., `repository: Repository`).

4. **Create Router**:
    - implement `axum::Router` configuration in `router.rs`.
    - define handlers: `pub async fn handler(State(state): State<AppState>, Json(payload): Json<Payload>) -> impl IntoResponse`.

## 3. Verification

1. **Compile**: `cargo check`.
2. **Lint**: `cargo clippy`.
3. **Test**: `cargo test`.
4. **Run**: `cargo run` (or `cargo run --bin [binary_name]`).

## 4. Documentation

- Add comments to public functions (/// doc comments).
- Update OpenAPI/Swagger if using utoipa.
