# Backend Agent - Tech Stack Reference

## Core Stack (Rust)
- **Framework**: Axum 0.7+
- **Runtime**: Tokio 1.0+ (Full features)
- **Database**: SQLx 0.7+ (PostgreSQL/SQLite, compile-time checked)
- **Serialization**: Serde, Serde JSON
- **Error Handling**: `thiserror` (lib), `anyhow` (app/cli)
- **Logging**: Tracing, Tracing Subscriber

## Architecture (Cratos Core)
- **Crate**: `crates/cratos-core`
- **API Layer**: `src/api` (Handlers, Extractors, Middleware)
- **Domain Layer**: `src/domain` (Business Logic, Pure Rust Types)
- **Infra Layer**: `src/infra` (Database, External Services)

## Security
- **Auth**: Axum Middleware + JWT/Argobap
- **Password**: Argon2 (via `argon2` crate)
- **Validation**: `validator` crate (struct validation)

## Testing
- **Unit**: `cargo test` (standard lib)
- **Integration**: `tower::Service` testing, `sqlx::test`

## Notable Crates
- `shuttle-runtime` (if applicable)
- `uuid` (v4, serde support)
- `chrono` or `time` (serde support)
- `reqwest` (HTTP Client)
