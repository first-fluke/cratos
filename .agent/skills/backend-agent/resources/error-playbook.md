# Error Playbook

Common Rust/Axum/SQLx errors and how to fix them.

## Compiler Errors

### "borrow of moved value"
- **Cause**: Variable ownership was transferred (moved) to a function or closure, and you tried to use it again.
- **Fix**:
    - Clone the value before moving: `value.clone()`
    - Pass by reference if possible: `&value`
    - Use `Arc` (Atomic Reference Counting) for shared ownership across threads/tasks.

### "lifetime mismatch"
- **Cause**: References outlive the data they point to.
- **Fix**:
    - Ensure data lives long enough (e.g., move into the struct/closure).
    - Use "owned" types (`String` instead of `&str`) in structs.
    - If needed, annotate lifetimes `'a`, but usually owned types are easier for application logic.

### "cannot infer type"
- **Cause**: Compiler needs more info.
- **Fix**: Explicitly specify types: `let x: Vec<i32> = ...`

## Axum Errors

### "the trait `Handler<...>` is not implemented for `...`"
- **Cause**: Handler function signature doesn't match what Axum expects.
- **Fix**:
    - Ensure all arguments implement `FromRequest` or `FromRequestParts`.
    - Ensure return type implements `IntoResponse`.
    - Common mistake: Extractors (like `Json`, `State`) must be in the correct order. `body` extractors (Json) must be **last**.

## SQLx Errors

### "error: defined in sqlx::query_as! macro ..."
- **Cause**: SQL query syntax error, or database schema doesn't match query.
- **Fix**:
    - Need a running database for `query_as!` macro to check against.
    - Check `.env` `DATABASE_URL`.
    - Run `sqlx migrate run` to ensure Schema is up to date.
    - Force offline mode: `cargo sqlx prepare` (if using offline feature).

### "mismatched types" in query_as!
- **Cause**: Rust struct fields don't match SQL column types (e.g. `i32` vs `i64`).
- **Fix**:
     - Cast in SQL: `SELECT count(*) as "count!: i64"`
     - Change Rust struct type to match DB (e.g. SQLite INTEGER is `i64`).
     - Handle nullables: Use `Option<T>` in Rust struct.
