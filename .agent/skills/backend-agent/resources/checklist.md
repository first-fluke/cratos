# Backend Agent - Self-Verification Checklist

Run through every item before confirming task completion.

## Rust Code Quality
- [ ] `cargo check` passes without errors
- [ ] `cargo clippy` passes (no warnings)
- [ ] `cargo fmt` has been run
- [ ] No `unwrap()` or `expect()` in production code (use `?` or explicit matching)

## API Implementation (Axum)
- [ ] Handlers use `Result<Json<T>, AppError>` return types
- [ ] State extracted via `State(AppState)` pattern
- [ ] Input validation (e.g., `validator` crate) applied to request bodies
- [ ] Correct HTTP status codes used (201 Created, 404 Not Found, etc.)

## Database (SQLx)
- [ ] Queries use `query!` or `query_as!` macros (compile-time checked)
- [ ] Migrations created for schema changes (`sqlx migrate add`)
- [ ] Transactions used for multi-step writes
- [ ] No N+1 query patterns (use joins)

## Security
- [ ] Auth middleware applied to protected routes
- [ ] Secrets loaded from config/env (never hardcoded)
- [ ] SQL injection impossible (due to SQLx usage)

## Testing
- [ ] Unit tests for domain logic
- [ ] Integration tests for API endpoints using `TestClient` or similar
- [ ] All tests pass (`cargo test`)
