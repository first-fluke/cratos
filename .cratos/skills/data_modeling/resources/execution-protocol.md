# Execution Protocol for Data Modeling

## 1. Requirement Analysis
- **Input**: User stories, feature specs, or "gap analysis" findings.
- **Output**: List of Entities (Nouns) and Relationships (Verbs).

## 2. Schema Definition
- **Action**:
    - Draft SQL `CREATE TABLE` statements.
    - Define Rust `struct` definitions (using `sqlx::FromRow`).
    - Define JSON/Serde mappings if API involved.

## 3. Migration Creation
- **Action**:
    - Create new migration file (timestamped).
    - Write Up/Down logic.
    - Ensure constraints (`NOT NULL`, `REFERENCES`) are strict.

## 4. Review
- **Checklist**:
    - [ ] IDs are UUID or BigInt?
    - [ ] Timestamps included?
    - [ ] Foreign Keys have indices?
    - [ ] Soft delete supported (if needed)?

