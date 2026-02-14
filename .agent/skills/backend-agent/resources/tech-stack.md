# Tech Stack

## Core Language
- **Rust** (Edition 2021)
- **Tokio**: Async Runtime

## Web Framework
- **Axum**: Ergonomic and modular web framework
- **Tower**: Middleware (Service) abstraction
- **Tower-HTTP**: HTTP middleware (Trace, CORS, etc.)

## Database
- **SQLx**: Async, pure Rust SQL crate (Compile-time checked queries)
- **SQLite**: Embedded database (via SQLx)
- **Bincode/Serde**: Serialization

## Serialization
- **Serde**: Serialization framework
- **Serde JSON**: JSON support

## Error Handling
- **ThisError**: Library error derivation
- **Anyhow**: Application error reporting

## Observability
- **Tracing**: Structured logging
- **Tracing Subscriber**: Log collection

## Testing
- **Mockall**: Mocking library
- **Tokio-test**: Async testing utilities
