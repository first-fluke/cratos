use super::GraphStore;
use crate::error::Result;

impl GraphStore {
    // ── Migrations ──────────────────────────────────────────────

    pub(crate) async fn run_migrations(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS turns (
                id          TEXT PRIMARY KEY,
                session_id  TEXT NOT NULL,
                role        TEXT NOT NULL,
                content     TEXT NOT NULL,
                summary     TEXT NOT NULL,
                turn_index  INTEGER NOT NULL,
                token_count INTEGER NOT NULL,
                created_at  TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_turns_session
             ON turns(session_id, turn_index)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS entities (
                id            TEXT PRIMARY KEY,
                name          TEXT NOT NULL UNIQUE,
                kind          TEXT NOT NULL,
                first_seen    TEXT NOT NULL,
                mention_count INTEGER NOT NULL DEFAULT 1
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS turn_entity_edges (
                turn_id   TEXT NOT NULL REFERENCES turns(id),
                entity_id TEXT NOT NULL REFERENCES entities(id),
                relevance REAL NOT NULL DEFAULT 1.0,
                PRIMARY KEY (turn_id, entity_id)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_edges_entity
             ON turn_entity_edges(entity_id)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS entity_cooccurrence (
                entity_a          TEXT NOT NULL REFERENCES entities(id),
                entity_b          TEXT NOT NULL REFERENCES entities(id),
                cooccurrence_count INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (entity_a, entity_b)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS entity_relations (
                from_entity_id TEXT NOT NULL REFERENCES entities(id),
                to_entity_id   TEXT NOT NULL REFERENCES entities(id),
                kind           TEXT NOT NULL,
                PRIMARY KEY (from_entity_id, to_entity_id, kind)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_entity_relations_to
             ON entity_relations(to_entity_id)",
        )
        .execute(&self.pool)
        .await?;

        // ── Explicit memories ────────────────────────────────────
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS explicit_memories (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL UNIQUE,
                content      TEXT NOT NULL,
                category     TEXT NOT NULL DEFAULT 'general',
                tags         TEXT NOT NULL DEFAULT '',
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL,
                access_count INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_explicit_name ON explicit_memories(name)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_explicit_category ON explicit_memories(category)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS memory_entity_edges (
                memory_id TEXT NOT NULL REFERENCES explicit_memories(id),
                entity_id TEXT NOT NULL REFERENCES entities(id),
                relevance REAL NOT NULL DEFAULT 1.0,
                PRIMARY KEY (memory_id, entity_id)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_mem_edges_entity ON memory_entity_edges(entity_id)",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
