# API Generation Workflows

Generate API clients from OpenAPI schemas for Cratos (Rust/Axum).

## OpenAPI to Client Pipeline

```bash
# 1. Generate OpenAPI schema from backend
mise run backend:gen:openapi

# 2. Generate TypeScript client for Web
mise run web:gen:api

# 3. Generate Client for Mobile
mise run mobile:gen:api
```

## Backend Tasks (Rust)

```toml
# crates/cratos-core/mise.toml
[tasks.gen:openapi]
description = "Generate OpenAPI schema via utoipa/axum"
run = "cargo run --bin openapi-gen > openapi.json"
```

## Web Tasks

```toml
# apps/web/mise.toml
[tasks.gen:api]
description = "Generate API client from OpenAPI"
depends = ["backend:gen:openapi"]
run = "bunx orval --config ./orval.config.ts"
```

## Mobile Tasks

```toml
# apps/mobile/mise.toml
[tasks.gen:api]
description = "Generate API client from OpenAPI"
depends = ["backend:gen:openapi"]
run = "dart run build_runner build"
```
