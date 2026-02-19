---
name: data_modeling
description: Design and validate data models for business domains.
version: 1.0.0
triggers:
  - "data modeling"
  - "design schema"
  - "entity relationship"
  - "create tables"
---

# Data Modeling

Design robust, scalable, and tailored data models.
Ensures data integrity, performance, and alignment with business rules.

## Role

- Translate business requirements into technical data structures (ERD, Schema).
- Define relationships (1:1, 1:N, N:M) and constraints (FK, Unique, Check).
- optimize for query performance (Indexes, Partitioning).
- Ensure compatibility with chosen DB (PostgreSQL, SQLite, etc.).

## Core Rules

1.  **Normalization**: Default to 3NF unless performance dictates denormalization.
2.  **Naming Convention**: Use `snake_case` for SQL/Rust. Table names plural (e.g., `users`).
3.  **Idempotency**: Migrations must be repeatable.
4.  **Auditability**: Include `created_at`, `updated_at` timestamps.

## Workflow

### Step 1: Conceptual Design

Identify Entities and Relationships from requirements.
Draw (or describe) the ERD.

### Step 2: Logical Schema Design

Define tables, columns, data types (Rust types mapping to SQL types).
Define Primary Keys and Foreign Keys.

### Step 3: Physical Implementation Plan

Create migration SQL or ORM structs (e.g., `sqlx`, `diesel`).
Define indexes for frequent access patterns.

### Step 4: Verification

Review against access patterns (Query Analysis).
Check for potential bottlenecks or circular dependencies.

## Resources

- **Protocol**: `resources/execution-protocol.md`
- **Examples**: `resources/examples.md`

