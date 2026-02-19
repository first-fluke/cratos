# Database Patterns (SQLite + SQLx)

Database migration workflows using SQLx and SQLite.
**Note**: Cratos uses SQLite, so no Docker container is required for the database.

## Migration Tasks

```toml
# crates/cratos-core/mise.toml
[tasks.db:create]
description = "Create database"
run = "cargo sqlx database create"

[tasks.db:migrate]
description = "Run SQLx migrations"
run = "cargo sqlx migrate run"

[tasks.db:add]
description = "Create new migration"
run = "cargo sqlx migrate add -r {{arg(name)}}"

[tasks.db:revert]
description = "Revert last migration"
run = "cargo sqlx migrate revert"

[tasks.db:reset]
description = "Reset database"
run = """
cargo sqlx database drop -y
cargo sqlx database create
cargo sqlx migrate run
"""

[tasks.db:prepare]
description = "Update offline query data for compile-time checking"
run = "cargo sqlx prepare -- --lib"
```

## Creating Migrations

```bash
# Create new migration
mise run db:add name=add_users_table

# Edit generated SQL file in migrations/ directory

# Apply migration
mise run db:migrate

# Update sqlx-data.json for compile-time verification
mise run db:prepare
```
