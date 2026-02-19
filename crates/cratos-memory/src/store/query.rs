use super::GraphStore;
use crate::error::Result;
use crate::types::{Entity, EntityKind, EntityRelation, ExplicitMemory, RelationKind};
use chrono::{DateTime, Utc};
use sqlx::Row;

impl GraphStore {
    // ── Entity Relations ────────────────────────────────────────

    /// Insert an entity relation. No-op if already exists.
    pub async fn insert_relation(&self, rel: &EntityRelation) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO entity_relations (from_entity_id, to_entity_id, kind)
             VALUES (?1, ?2, ?3)",
        )
        .bind(&rel.from_entity_id)
        .bind(&rel.to_entity_id)
        .bind(rel.kind.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all relations where the given entity is the source.
    pub async fn get_relations_from_entity(&self, entity_id: &str) -> Result<Vec<EntityRelation>> {
        let rows = sqlx::query(
            "SELECT from_entity_id, to_entity_id, kind
             FROM entity_relations WHERE from_entity_id = ?1",
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| EntityRelation {
                from_entity_id: r.get("from_entity_id"),
                to_entity_id: r.get("to_entity_id"),
                kind: RelationKind::from_str_lossy(r.get("kind")),
            })
            .collect())
    }

    /// Get all relations where the given entity is the target.
    pub async fn get_relations_to_entity(&self, entity_id: &str) -> Result<Vec<EntityRelation>> {
        let rows = sqlx::query(
            "SELECT from_entity_id, to_entity_id, kind
             FROM entity_relations WHERE to_entity_id = ?1",
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| EntityRelation {
                from_entity_id: r.get("from_entity_id"),
                to_entity_id: r.get("to_entity_id"),
                kind: RelationKind::from_str_lossy(r.get("kind")),
            })
            .collect())
    }

    /// List all entity relations (for graph visualization).
    pub async fn list_all_relations(&self, limit: u32) -> Result<Vec<EntityRelation>> {
        let rows = sqlx::query(
            "SELECT from_entity_id, to_entity_id, kind
             FROM entity_relations
             LIMIT ?1",
        )
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| EntityRelation {
                from_entity_id: r.get("from_entity_id"),
                to_entity_id: r.get("to_entity_id"),
                kind: RelationKind::from_str_lossy(r.get("kind")),
            })
            .collect())
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

    // ── Graph Data Export ─────────────────────────────────────

    /// List all entities (for graph visualization).
    pub async fn list_all_entities(&self, limit: u32) -> Result<Vec<Entity>> {
        let rows = sqlx::query(
            "SELECT id, name, kind, first_seen, mention_count
             FROM entities
             ORDER BY mention_count DESC
             LIMIT ?1",
        )
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|r| {
                let kind_str: String = r.try_get("kind")?;
                let first_seen_str: String = r.try_get("first_seen")?;
                Ok(Entity {
                    id: r.try_get("id")?,
                    name: r.try_get("name")?,
                    kind: EntityKind::from_str_lossy(&kind_str),
                    first_seen: DateTime::parse_from_rfc3339(&first_seen_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    mention_count: r.try_get::<i32, _>("mention_count")? as u32,
                })
            })
            .collect()
    }

    /// Get all co-occurrence edges (for graph visualization).
    pub async fn list_all_cooccurrences(&self, limit: u32) -> Result<Vec<(String, String, u32)>> {
        let rows = sqlx::query(
            "SELECT entity_a, entity_b, cooccurrence_count
             FROM entity_cooccurrence
             ORDER BY cooccurrence_count DESC
             LIMIT ?1",
        )
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| {
                let a: String = r.get("entity_a");
                let b: String = r.get("entity_b");
                let count: i32 = r.get("cooccurrence_count");
                (a, b, count as u32)
            })
            .collect())
    }

    /// Search explicit memories by name/content LIKE matching.
    pub async fn search_explicit(
        &self,
        query: &str,
        category: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ExplicitMemory>> {
        let pattern = format!("%{query}%");
        let rows = if let Some(cat) = category {
            sqlx::query(
                "SELECT id, name, content, category, tags, created_at, updated_at, access_count
                 FROM explicit_memories
                 WHERE category = ?1 AND (name LIKE ?2 OR content LIKE ?2)
                 ORDER BY access_count DESC, updated_at DESC
                 LIMIT ?3",
            )
            .bind(cat)
            .bind(&pattern)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, name, content, category, tags, created_at, updated_at, access_count
                 FROM explicit_memories
                 WHERE name LIKE ?1 OR content LIKE ?1
                 ORDER BY access_count DESC, updated_at DESC
                 LIMIT ?2",
            )
            .bind(&pattern)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        rows.iter().map(Self::row_to_explicit_memory).collect()
    }

    /// List explicit memories, optionally filtered by category.
    pub async fn list_explicit(
        &self,
        category: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ExplicitMemory>> {
        let rows = if let Some(cat) = category {
            sqlx::query(
                "SELECT id, name, content, category, tags, created_at, updated_at, access_count
                 FROM explicit_memories
                 WHERE category = ?1
                 ORDER BY updated_at DESC
                 LIMIT ?2",
            )
            .bind(cat)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, name, content, category, tags, created_at, updated_at, access_count
                 FROM explicit_memories
                 ORDER BY updated_at DESC
                 LIMIT ?1",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        rows.iter().map(Self::row_to_explicit_memory).collect()
    }

    /// Get explicit memories linked to an entity via memory_entity_edges.
    pub async fn get_explicit_by_entity(&self, entity_id: &str) -> Result<Vec<ExplicitMemory>> {
        let rows = sqlx::query(
            "SELECT m.id, m.name, m.content, m.category, m.tags,
                    m.created_at, m.updated_at, m.access_count
             FROM explicit_memories m
             JOIN memory_entity_edges me ON me.memory_id = m.id
             WHERE me.entity_id = ?1
             ORDER BY me.relevance DESC",
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::row_to_explicit_memory).collect()
    }
}
