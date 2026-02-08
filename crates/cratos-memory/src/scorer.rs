//! Hybrid scoring — combines embedding similarity, proximity, and entity overlap.

/// Weights for the three scoring factors.
pub struct ScoringWeights {
    /// Weight for embedding similarity (default 0.5)
    pub embedding: f32,
    /// Weight for session proximity (default 0.3)
    pub proximity: f32,
    /// Weight for entity overlap (default 0.2)
    pub entity_overlap: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            embedding: 0.5,
            proximity: 0.3,
            entity_overlap: 0.2,
        }
    }
}

/// Compute a hybrid score for a candidate turn.
///
/// - `embedding_sim`: cosine similarity from VectorIndex (0.0–1.0)
/// - `proximity`: inverse distance to a seed turn in the same session (0.0–1.0)
/// - `entity_overlap`: fraction of query entities matched (0.0–1.0)
pub fn hybrid_score(
    weights: &ScoringWeights,
    embedding_sim: f32,
    proximity: f32,
    entity_overlap: f32,
) -> f32 {
    weights.embedding * embedding_sim
        + weights.proximity * proximity
        + weights.entity_overlap * entity_overlap
}

/// Compute proximity score between two turns in the same session.
///
/// Returns 1.0 when adjacent, decays towards 0.0 with increasing distance.
/// Returns 0.0 for turns in different sessions.
pub fn proximity_score(
    seed_session: &str,
    seed_index: u32,
    candidate_session: &str,
    candidate_index: u32,
) -> f32 {
    if seed_session != candidate_session {
        return 0.0;
    }
    let distance = (seed_index as f32 - candidate_index as f32).abs();
    1.0 / (1.0 + distance)
}

/// Compute entity overlap ratio between query entities and candidate entities.
pub fn entity_overlap_score(query_entities: &[String], candidate_entities: &[String]) -> f32 {
    if query_entities.is_empty() {
        return 0.0;
    }
    let matches = query_entities
        .iter()
        .filter(|qe| candidate_entities.iter().any(|ce| ce == *qe))
        .count();
    matches as f32 / query_entities.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_score_default_weights() {
        let w = ScoringWeights::default();
        let score = hybrid_score(&w, 0.8, 0.5, 1.0);
        // 0.5*0.8 + 0.3*0.5 + 0.2*1.0 = 0.4 + 0.15 + 0.2 = 0.75
        assert!((score - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_proximity_same_session() {
        // Adjacent turns
        assert!((proximity_score("s1", 3, "s1", 4) - 0.5).abs() < 0.001);
        // Same turn
        assert!((proximity_score("s1", 3, "s1", 3) - 1.0).abs() < 0.001);
        // Far apart
        assert!(proximity_score("s1", 0, "s1", 100) < 0.02);
    }

    #[test]
    fn test_proximity_different_session() {
        assert_eq!(proximity_score("s1", 3, "s2", 3), 0.0);
    }

    #[test]
    fn test_entity_overlap() {
        let query = vec!["a".into(), "b".into(), "c".into()];
        let candidate = vec!["b".into(), "c".into(), "d".into()];
        let score = entity_overlap_score(&query, &candidate);
        assert!((score - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_entity_overlap_empty() {
        assert_eq!(entity_overlap_score(&[], &["a".into()]), 0.0);
    }
}
