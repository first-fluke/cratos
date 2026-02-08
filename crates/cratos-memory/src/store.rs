//! GraphStore — SQLite persistence for the entity-turn graph.
//!
//! Tables: `turns`, `entities`, `turn_entity_edges`, `entity_cooccurrence`.

use crate::error::{Error, Result};
use crate::types::{Entity, EntityKind, Turn, TurnEntityEdge, TurnRole};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use tracing::{debug, info};

/// SQLite-backed graph store.
#[derive(Clone)]
pub struct GraphStore {
    pool: SqlitePool,
}

impl GraphStore {
    /// Open (or create) a graph store at the given path.
    pub async fn from_path(db_path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Internal(format!("mkdir: {e}")))?;
        }
        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;

        // Enable WAL for read/write concurrency
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await?;

        let store = Self { pool };
        store.run_migrations().await?;
        info!("Graph store initialized at {}", db_path.display());
        Ok(store)
    }

    /// In-memory store (for tests).
    pub async fn in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let store = Self { pool };
        store.run_migrations().await?;
        debug!("In-memory graph store initialized");
        Ok(store)
    }

    // ── Migrations ──────────────────────────────────────────────

    async fn run_migrations(&self) -> Result<()> {
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

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name)",
        )
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

        Ok(())
    }

    // ── Turns ───────────────────────────────────────────────────

    /// Insert a turn. No-op if already exists (idempotent).
    pub async fn insert_turn(&self, turn: &Turn) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO turns
             (id, session_id, role, content, summary, turn_index, token_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&turn.id)
        .bind(&turn.session_id)
        .bind(turn.role.to_string())
        .bind(&turn.content)
        .bind(&turn.summary)
        .bind(turn.turn_index)
        .bind(turn.token_count)
        .bind(turn.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all turns for a session, ordered by turn_index.
    pub async fn get_turns_by_session(&self, session_id: &str) -> Result<Vec<Turn>> {
        let rows = sqlx::query(
            "SELECT id, session_id, role, content, summary, turn_index, token_count, created_at
             FROM turns WHERE session_id = ?1 ORDER BY turn_index",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::row_to_turn).collect()
    }

    /// Get a turn by ID.
    pub async fn get_turn(&self, id: &str) -> Result<Option<Turn>> {
        let row = sqlx::query(
            "SELECT id, session_id, role, content, summary, turn_index, token_count, created_at
             FROM turns WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(Self::row_to_turn).transpose()
    }

    /// Get multiple turns by their IDs.
    ///
    /// Uses individual parameterized queries to avoid SQL injection risks.
    pub async fn get_turns_by_ids(&self, ids: &[String]) -> Result<Vec<Turn>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Use parameterized queries (one per ID) to avoid SQL injection.
        // SQLite doesn't support array bind parameters natively.
        let mut turns = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(turn) = self.get_turn(id).await? {
                turns.push(turn);
            }
        }
        turns.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(turns)
    }

    /// Maximum turn_index for a session (to detect new turns).
    pub async fn max_turn_index(&self, session_id: &str) -> Result<Option<u32>> {
        let row = sqlx::query(
            "SELECT MAX(turn_index) as max_idx FROM turns WHERE session_id = ?1",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.try_get::<Option<i32>, _>("max_idx")?.map(|v| v as u32))
    }

    fn row_to_turn(row: &sqlx::sqlite::SqliteRow) -> Result<Turn> {
        let role_str: String = row.try_get("role")?;
        let created_str: String = row.try_get("created_at")?;
        Ok(Turn {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            role: match role_str.as_str() {
                "user" => TurnRole::User,
                _ => TurnRole::Assistant,
            },
            content: row.try_get("content")?,
            summary: row.try_get("summary")?,
            turn_index: row.try_get::<i32, _>("turn_index")? as u32,
            token_count: row.try_get::<i32, _>("token_count")? as u32,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    // ── Entities ────────────────────────────────────────────────

    /// Insert or update an entity. Increments `mention_count` on conflict.
    pub async fn upsert_entity(&self, entity: &Entity) -> Result<()> {
        sqlx::query(
            "INSERT INTO entities (id, name, kind, first_seen, mention_count)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(name) DO UPDATE SET
                mention_count = mention_count + 1",
        )
        .bind(&entity.id)
        .bind(&entity.name)
        .bind(entity.kind.to_string())
        .bind(entity.first_seen.to_rfc3339())
        .bind(entity.mention_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Look up an entity by normalized name.
    pub async fn get_entity_by_name(&self, name: &str) -> Result<Option<Entity>> {
        let row = sqlx::query(
            "SELECT id, name, kind, first_seen, mention_count
             FROM entities WHERE name = ?1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(Self::row_to_entity).transpose()
    }

    fn row_to_entity(row: &sqlx::sqlite::SqliteRow) -> Result<Entity> {
        let kind_str: String = row.try_get("kind")?;
        let seen_str: String = row.try_get("first_seen")?;
        Ok(Entity {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            kind: EntityKind::from_str_lossy(&kind_str),
            first_seen: DateTime::parse_from_rfc3339(&seen_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            mention_count: row.try_get::<i32, _>("mention_count")? as u32,
        })
    }

    // ── Edges ───────────────────────────────────────────────────

    /// Insert a turn-entity edge. No-op if already exists.
    pub async fn insert_edge(&self, edge: &TurnEntityEdge) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO turn_entity_edges (turn_id, entity_id, relevance)
             VALUES (?1, ?2, ?3)",
        )
        .bind(&edge.turn_id)
        .bind(&edge.entity_id)
        .bind(edge.relevance)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all entity IDs linked to a turn.
    pub async fn get_entities_for_turn(&self, turn_id: &str) -> Result<Vec<Entity>> {
        let rows = sqlx::query(
            "SELECT e.id, e.name, e.kind, e.first_seen, e.mention_count
             FROM entities e
             JOIN turn_entity_edges te ON te.entity_id = e.id
             WHERE te.turn_id = ?1",
        )
        .bind(turn_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::row_to_entity).collect()
    }

    /// Get all turn IDs linked to an entity (BFS 1-hop).
    pub async fn get_turn_ids_for_entity(&self, entity_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT turn_id FROM turn_entity_edges WHERE entity_id = ?1",
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| r.get("turn_id")).collect())
    }

    // ── Co-occurrence ───────────────────────────────────────────

    /// Update co-occurrence counts for a set of entities appearing in the same turn.
    pub async fn update_cooccurrence(&self, entity_ids: &[String]) -> Result<()> {
        for i in 0..entity_ids.len() {
            for j in (i + 1)..entity_ids.len() {
                let (a, b) = if entity_ids[i] < entity_ids[j] {
                    (&entity_ids[i], &entity_ids[j])
                } else {
                    (&entity_ids[j], &entity_ids[i])
                };
                sqlx::query(
                    "INSERT INTO entity_cooccurrence (entity_a, entity_b, cooccurrence_count)
                     VALUES (?1, ?2, 1)
                     ON CONFLICT(entity_a, entity_b) DO UPDATE SET
                        cooccurrence_count = cooccurrence_count + 1",
                )
                .bind(a)
                .bind(b)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Get entities that frequently co-occur with the given entity.
    pub async fn get_cooccurring_entities(
        &self,
        entity_id: &str,
        limit: u32,
    ) -> Result<Vec<(String, u32)>> {
        let rows = sqlx::query(
            "SELECT
                CASE WHEN entity_a = ?1 THEN entity_b ELSE entity_a END as other_id,
                cooccurrence_count
             FROM entity_cooccurrence
             WHERE entity_a = ?1 OR entity_b = ?1
             ORDER BY cooccurrence_count DESC
             LIMIT ?2",
        )
        .bind(entity_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| {
                let other: String = r.get("other_id");
                let count: i32 = r.get("cooccurrence_count");
                (other, count as u32)
            })
            .collect())
    }

    // ── Stats ───────────────────────────────────────────────────

    /// Total number of turns stored.
    pub async fn turn_count(&self) -> Result<u32> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM turns")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i32, _>("cnt")? as u32)
    }

    /// Total number of entities stored.
    pub async fn entity_count(&self) -> Result<u32> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM entities")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i32, _>("cnt")? as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    async fn test_store() -> GraphStore {
        GraphStore::in_memory().await.unwrap()
    }

    fn make_turn(id: &str, session: &str, idx: u32, role: TurnRole) -> Turn {
        Turn {
            id: id.into(),
            session_id: session.into(),
            role,
            content: format!("content {id}"),
            summary: format!("summary {id}"),
            turn_index: idx,
            token_count: 10,
            created_at: Utc::now(),
        }
    }

    fn make_entity(id: &str, name: &str, kind: EntityKind) -> Entity {
        Entity {
            id: id.into(),
            name: name.into(),
            kind,
            first_seen: Utc::now(),
            mention_count: 1,
        }
    }

    #[tokio::test]
    async fn test_insert_and_get_turn() {
        let store = test_store().await;
        let turn = make_turn("t1", "s1", 0, TurnRole::User);
        store.insert_turn(&turn).await.unwrap();

        let got = store.get_turn("t1").await.unwrap().unwrap();
        assert_eq!(got.id, "t1");
        assert_eq!(got.session_id, "s1");
        assert_eq!(got.role, TurnRole::User);
        assert_eq!(got.turn_index, 0);
    }

    #[tokio::test]
    async fn test_idempotent_insert() {
        let store = test_store().await;
        let turn = make_turn("t1", "s1", 0, TurnRole::User);
        store.insert_turn(&turn).await.unwrap();
        store.insert_turn(&turn).await.unwrap(); // no error
        assert_eq!(store.turn_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_turns_by_session() {
        let store = test_store().await;
        store.insert_turn(&make_turn("a", "s1", 0, TurnRole::User)).await.unwrap();
        store.insert_turn(&make_turn("b", "s1", 1, TurnRole::Assistant)).await.unwrap();
        store.insert_turn(&make_turn("c", "s2", 0, TurnRole::User)).await.unwrap();

        let s1 = store.get_turns_by_session("s1").await.unwrap();
        assert_eq!(s1.len(), 2);
        assert_eq!(s1[0].turn_index, 0);
        assert_eq!(s1[1].turn_index, 1);
    }

    #[tokio::test]
    async fn test_max_turn_index() {
        let store = test_store().await;
        assert_eq!(store.max_turn_index("s1").await.unwrap(), None);

        store.insert_turn(&make_turn("a", "s1", 0, TurnRole::User)).await.unwrap();
        store.insert_turn(&make_turn("b", "s1", 3, TurnRole::Assistant)).await.unwrap();
        assert_eq!(store.max_turn_index("s1").await.unwrap(), Some(3));
    }

    #[tokio::test]
    async fn test_upsert_entity() {
        let store = test_store().await;
        let ent = make_entity("e1", "orchestrator.rs", EntityKind::File);
        store.upsert_entity(&ent).await.unwrap();

        let got = store.get_entity_by_name("orchestrator.rs").await.unwrap().unwrap();
        assert_eq!(got.kind, EntityKind::File);
        assert_eq!(got.mention_count, 1);

        // Upsert again increments count
        let ent2 = make_entity("e1-dup", "orchestrator.rs", EntityKind::File);
        store.upsert_entity(&ent2).await.unwrap();
        let got2 = store.get_entity_by_name("orchestrator.rs").await.unwrap().unwrap();
        assert_eq!(got2.mention_count, 2);
    }

    #[tokio::test]
    async fn test_edges_and_graph_traversal() {
        let store = test_store().await;
        let turn = make_turn("t1", "s1", 0, TurnRole::User);
        let ent = make_entity("e1", "foo.rs", EntityKind::File);
        store.insert_turn(&turn).await.unwrap();
        store.upsert_entity(&ent).await.unwrap();

        let edge = TurnEntityEdge {
            turn_id: "t1".into(),
            entity_id: "e1".into(),
            relevance: 0.9,
        };
        store.insert_edge(&edge).await.unwrap();

        // Turn → entities
        let entities = store.get_entities_for_turn("t1").await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "foo.rs");

        // Entity → turns
        let turn_ids = store.get_turn_ids_for_entity("e1").await.unwrap();
        assert_eq!(turn_ids, vec!["t1"]);
    }

    #[tokio::test]
    async fn test_cooccurrence() {
        let store = test_store().await;
        // Must insert entities first (FK constraint)
        store.upsert_entity(&make_entity("e1", "a.rs", EntityKind::File)).await.unwrap();
        store.upsert_entity(&make_entity("e2", "b.rs", EntityKind::File)).await.unwrap();
        store.upsert_entity(&make_entity("e3", "c.rs", EntityKind::File)).await.unwrap();

        let ids = vec!["e1".into(), "e2".into(), "e3".into()];
        store.update_cooccurrence(&ids).await.unwrap();
        store.update_cooccurrence(&["e1".into(), "e2".into()]).await.unwrap();

        let co = store.get_cooccurring_entities("e1", 10).await.unwrap();
        // e2 should have count 2 (appeared with e1 twice)
        let e2 = co.iter().find(|(id, _)| id == "e2").unwrap();
        assert_eq!(e2.1, 2);
        // e3 should have count 1
        let e3 = co.iter().find(|(id, _)| id == "e3").unwrap();
        assert_eq!(e3.1, 1);
    }

    #[tokio::test]
    async fn test_get_turns_by_ids() {
        let store = test_store().await;
        store.insert_turn(&make_turn("t1", "s1", 0, TurnRole::User)).await.unwrap();
        store.insert_turn(&make_turn("t2", "s1", 1, TurnRole::Assistant)).await.unwrap();
        store.insert_turn(&make_turn("t3", "s1", 2, TurnRole::User)).await.unwrap();

        let turns = store
            .get_turns_by_ids(&["t1".into(), "t3".into()])
            .await
            .unwrap();
        assert_eq!(turns.len(), 2);

        // Empty list
        let empty = store.get_turns_by_ids(&[]).await.unwrap();
        assert!(empty.is_empty());
    }
}
