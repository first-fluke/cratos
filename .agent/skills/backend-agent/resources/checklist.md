# pre-commit Checklist

Before submitting your code, verify the following:

## Compilation & formatting
- [ ] Code compiles without errors (`cargo check`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No strict clippy warnings (`cargo clippy -- -D warnings`)

## Type Safety & Error Handling
- [ ] No `unwrap()` or `expect()` used in production code paths (use `?` or `match`)
- [ ] All public functions have type hints (enforced by Rust)
- [ ] SQL queries use `sqlx::query_as!` (macros) for compile-time verification where possible

## Testing
- [ ] Unit tests added for new business logic
- [ ] `cargo test` passes
- [ ] Integration tests added for DB interactions (if applicable)

## Architecture
- [ ] Business logic is in `Service`, not `Router`
- [ ] Database queries are in `Repository`, not `Service`
- [ ] API Endpoints return appropriate HTTP status codes
- [ ] Secrets (keys, passwords) are NOT hardcoded
