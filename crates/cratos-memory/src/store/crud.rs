use super::GraphStore;
use crate::error::Result;
use crate::types::{Entity, EntityKind, ExplicitMemory, Turn, TurnEntityEdge, TurnRole};
use chrono::{DateTime, Utc};
use sqlx::Row;

impl GraphStore {
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
    pub async fn get_turns_by_ids(&self, ids: &[String]) -> Result<Vec<Turn>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
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
        let row = sqlx::query("SELECT MAX(turn_index) as max_idx FROM turns WHERE session_id = ?1")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get::<Option<i32>, _>("max_idx")?.map(|v| v as u32))
    }

    pub(crate) fn row_to_turn(row: &sqlx::sqlite::SqliteRow) -> Result<Turn> {
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

    pub(crate) fn row_to_entity(row: &sqlx::sqlite::SqliteRow) -> Result<Entity> {
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
        let rows = sqlx::query("SELECT turn_id FROM turn_entity_edges WHERE entity_id = ?1")
            .bind(entity_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.iter().map(|r| r.get("turn_id")).collect())
    }

    // ── Explicit Memories ─────────────────────────────────────

    /// Save an explicit memory (INSERT OR REPLACE).
    pub async fn save_explicit_memory(&self, mem: &ExplicitMemory) -> Result<()> {
        let tags_str = mem.tags.join(",");
        sqlx::query(
            "INSERT INTO explicit_memories
             (id, name, content, category, tags, created_at, updated_at, access_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(name) DO UPDATE SET
                content = excluded.content,
                category = excluded.category,
                tags = excluded.tags,
                updated_at = excluded.updated_at",
        )
        .bind(&mem.id)
        .bind(&mem.name)
        .bind(&mem.content)
        .bind(&mem.category)
        .bind(&tags_str)
        .bind(mem.created_at.to_rfc3339())
        .bind(mem.updated_at.to_rfc3339())
        .bind(mem.access_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get an explicit memory by name.
    pub async fn get_explicit_by_name(&self, name: &str) -> Result<Option<ExplicitMemory>> {
        let row = sqlx::query(
            "SELECT id, name, content, category, tags, created_at, updated_at, access_count
             FROM explicit_memories WHERE name = ?1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(Self::row_to_explicit_memory).transpose()
    }

    /// Get an explicit memory by ID.
    pub async fn get_explicit_by_id(&self, id: &str) -> Result<Option<ExplicitMemory>> {
        let row = sqlx::query(
            "SELECT id, name, content, category, tags, created_at, updated_at, access_count
             FROM explicit_memories WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(Self::row_to_explicit_memory).transpose()
    }

    /// Delete an explicit memory by name. Returns true if a row was deleted.
    pub async fn delete_explicit(&self, name: &str) -> Result<bool> {
        // First get the ID for cascade cleanup
        let id = sqlx::query("SELECT id FROM explicit_memories WHERE name = ?1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = id {
            let mem_id: String = row.try_get("id")?;
            // Delete entity edges first
            sqlx::query("DELETE FROM memory_entity_edges WHERE memory_id = ?1")
                .bind(&mem_id)
                .execute(&self.pool)
                .await?;
            // Delete the memory
            let result = sqlx::query("DELETE FROM explicit_memories WHERE id = ?1")
                .bind(&mem_id)
                .execute(&self.pool)
                .await?;
            Ok(result.rows_affected() > 0)
        } else {
            Ok(false)
        }
    }

    /// Increment the access count for a memory.
    pub async fn increment_access_count(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE explicit_memories SET access_count = access_count + 1 WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Insert an edge linking an explicit memory to an entity.
    pub async fn insert_memory_entity_edge(
        &self,
        memory_id: &str,
        entity_id: &str,
        relevance: f32,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO memory_entity_edges (memory_id, entity_id, relevance)
             VALUES (?1, ?2, ?3)",
        )
        .bind(memory_id)
        .bind(entity_id)
        .bind(relevance)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) fn row_to_explicit_memory(row: &sqlx::sqlite::SqliteRow) -> Result<ExplicitMemory> {
        let tags_str: String = row.try_get("tags")?;
        let created_str: String = row.try_get("created_at")?;
        let updated_str: String = row.try_get("updated_at")?;
        Ok(ExplicitMemory {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            content: row.try_get("content")?,
            category: row.try_get("category")?,
            tags: if tags_str.is_empty() {
                Vec::new()
            } else {
                tags_str.split(',').map(|s| s.trim().to_string()).collect()
            },
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            access_count: row.try_get::<i32, _>("access_count")? as u32,
        })
    }
}
