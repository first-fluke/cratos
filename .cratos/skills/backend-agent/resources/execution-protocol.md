# Backend Agent - Execution Protocol

## Step 0: Prepare
1. **Load Protocols**:
   - `../_shared/multi-review-protocol.md`
   - `../_shared/quality-principles.md`
   - `../_shared/phase-gates.md`
   - `../_shared/memory-protocol.md` (CLI mode)
2. **Context**: Use `get_symbols_overview("crates/cratos-core/src")` to understand current structures.
3. **Memory**: Create `session-backend-{id}.md` (if leading) or update `progress-{id}.md`.
4. **Plan**: Identify new models, routes, and migrations needed.

## Step 1: Analyze
- Read requirements.
- Locate relevant `api/`, `domain/`, `infra/` modules.
- Check `crates/cratos-core/Cargo.toml` for dependencies.

## Step 2: Plan (Rust/Axum)
- **Domain**: Define `struct`s and logic in `src/domain`.
- **Database**: Plan SQLx migrations (`migrations/TIMESTAMP_name.sql`).
- **API**: Define handlers and request/response types in `src/api`.
- **Tests**: Plan unit and integration tests.

## Step 3: Implement
1. **Migration**: Create and run SQL migration.
2. **Domain**: internal types and trait definitions.
3. **Repository (Infra)**: Implement `sqlx` logic.
4. **Service/Handler (API)**: Implement business logic and HTTP handlers.
5. **Router**: Register new routes in `app.rs` or module root.

## Step 4: Verify
- Run `cargo fmt`.
- Run `cargo clippy`.
- Run `cargo test`.
- Verify strict adherence to `resources/checklist.md`.
- **Memory**: Write `result-{id}.md` with completion status.

## On Error
See `resources/error-playbook.md`.
