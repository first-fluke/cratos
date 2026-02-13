//! Graph RAG retriever — finds relevant past turns for a query.
//!
//! Algorithm:
//! 1. Extract entities from the query
//! 2. Find seed turns via entity graph lookup
//! 3. BFS 1-hop: expand from seed entities to neighbouring turns
//! 4. Score all candidates with hybrid scoring
//! 5. Select top turns within the token budget

use crate::extractor;
use crate::scorer::{self, ScoringWeights};
use crate::store::GraphStore;
use crate::types::RetrievedTurn;
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// Maximum vector search results used as seed turns for graph expansion.
/// Higher values improve recall at the cost of more graph traversal.
const VECTOR_SEED_LIMIT: usize = 20;

/// Baseline relevance score for entity-matched turns without embedding similarity.
/// Set below typical embedding scores (0.5–1.0) so graph-only matches rank lower
/// than embedding-confirmed matches in the final hybrid score.
const BASE_ENTITY_MATCH_SCORE: f32 = 0.3;

/// Retrieves relevant past turns from the graph.
pub struct GraphRagRetriever {
    store: GraphStore,
    weights: ScoringWeights,
    /// Optional: vector search callback for embedding-based seed selection.
    vector_search: Option<Box<dyn VectorSearch>>,
}

/// Trait for vector-based seed search.
///
/// Decouples from concrete VectorIndex + EmbeddingProvider.
#[async_trait::async_trait]
pub trait VectorSearch: Send + Sync {
    /// Search for the top-K most similar turn IDs given a query string.
    /// Returns `(turn_id, similarity_score)` pairs.
    async fn search(&self, query: &str, top_k: usize) -> crate::Result<Vec<(String, f32)>>;
}

impl GraphRagRetriever {
    /// Create a retriever without vector search (entity-graph only).
    pub fn new(store: GraphStore) -> Self {
        Self {
            store,
            weights: ScoringWeights::default(),
            vector_search: None,
        }
    }

    /// Create a retriever with vector search support.
    pub fn with_vector_search(store: GraphStore, vector_search: Box<dyn VectorSearch>) -> Self {
        Self {
            store,
            weights: ScoringWeights::default(),
            vector_search: Some(vector_search),
        }
    }

    /// Retrieve relevant turns for a query.
    ///
    /// - `max_turns`: maximum number of turns to return
    /// - `max_tokens`: token budget (stops adding turns when exceeded)
    pub async fn retrieve(
        &self,
        query: &str,
        max_turns: usize,
        max_tokens: u32,
    ) -> crate::Result<Vec<RetrievedTurn>> {
        // 1. Extract entities from the query
        let query_entities = extractor::extract(query);
        let query_entity_names: Vec<String> =
            query_entities.iter().map(|e| e.name.clone()).collect();

        // 2. Gather seed turn IDs
        let mut seed_scores: HashMap<String, f32> = HashMap::new();

        // 2a. Vector search seeds (if available)
        if let Some(vs) = &self.vector_search {
            let results = vs.search(query, VECTOR_SEED_LIMIT).await?;
            for (turn_id, sim) in results {
                seed_scores.insert(turn_id, sim);
            }
        }

        // 2b. Entity-graph seeds: find turns mentioning query entities
        for ext in &query_entities {
            if let Some(entity) = self.store.get_entity_by_name(&ext.name).await? {
                let turn_ids = self.store.get_turn_ids_for_entity(&entity.id).await?;
                for tid in turn_ids {
                    seed_scores.entry(tid).or_insert(BASE_ENTITY_MATCH_SCORE);
                }
            }
        }

        if seed_scores.is_empty() {
            debug!("No seed turns found for query");
            return Ok(Vec::new());
        }

        // 3. BFS 1-hop: from seed turns, find their entities, then other turns
        let mut candidate_ids: HashSet<String> = seed_scores.keys().cloned().collect();

        let seed_ids: Vec<String> = seed_scores.keys().cloned().collect();
        for seed_id in &seed_ids {
            let entities = self.store.get_entities_for_turn(seed_id).await?;
            for entity in &entities {
                let neighbour_ids = self.store.get_turn_ids_for_entity(&entity.id).await?;
                for nid in neighbour_ids {
                    candidate_ids.insert(nid);
                }
            }
        }

        // 4. Load candidate turns and score them
        let all_ids: Vec<String> = candidate_ids.into_iter().collect();
        let turns = self.store.get_turns_by_ids(&all_ids).await?;

        // Find a representative seed for proximity computation
        let seed_turn = if !seed_ids.is_empty() {
            self.store.get_turn(&seed_ids[0]).await?
        } else {
            None
        };

        let mut scored: Vec<RetrievedTurn> = Vec::with_capacity(turns.len());
        for turn in turns {
            let embedding_sim = seed_scores.get(&turn.id).copied().unwrap_or(0.0);

            let proximity = seed_turn.as_ref().map_or(0.0, |st| {
                scorer::proximity_score(
                    &st.session_id,
                    st.turn_index,
                    &turn.session_id,
                    turn.turn_index,
                )
            });

            // Get this turn's entity names for overlap computation
            let turn_entities = self.store.get_entities_for_turn(&turn.id).await?;
            let turn_entity_names: Vec<String> =
                turn_entities.iter().map(|e| e.name.clone()).collect();
            let overlap = scorer::entity_overlap_score(&query_entity_names, &turn_entity_names);

            let score = scorer::hybrid_score(&self.weights, embedding_sim, proximity, overlap);

            scored.push(RetrievedTurn {
                turn,
                score,
                matched_entities: turn_entity_names,
            });
        }

        // 5. Sort by score descending, then apply token budget
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut selected = Vec::new();
        let mut token_sum = 0u32;
        for rt in scored {
            if selected.len() >= max_turns {
                break;
            }
            if token_sum + rt.turn.token_count > max_tokens && !selected.is_empty() {
                break;
            }
            token_sum += rt.turn.token_count;
            selected.push(rt);
        }

        // Re-sort by turn_index for chronological order
        selected.sort_by_key(|rt| (rt.turn.session_id.clone(), rt.turn.turn_index));

        debug!(
            query_entities = query_entity_names.len(),
            candidates = all_ids.len(),
            selected = selected.len(),
            token_sum,
            "Graph RAG retrieval complete"
        );

        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::TurnIndexer;
    use crate::store::GraphStore;
    use cratos_llm::Message;

    #[tokio::test]
    async fn test_retrieve_by_entity() {
        let store = GraphStore::in_memory().await.unwrap();
        let indexer = TurnIndexer::new(store.clone());

        // Index some conversations
        let messages = vec![
            Message::user("Fix the bug in orchestrator.rs"),
            Message::assistant("I found the issue in orchestrator.rs, fixing now"),
            Message::user("Now update store.rs with new fields"),
            Message::assistant("Updated store.rs successfully"),
        ];
        indexer.index_session("s1", &messages).await.unwrap();

        // Query about orchestrator
        let retriever = GraphRagRetriever::new(store);
        let results = retriever
            .retrieve("problem in orchestrator.rs", 10, 10000)
            .await
            .unwrap();

        assert!(!results.is_empty());
        // Should find turns mentioning orchestrator.rs
        assert!(results
            .iter()
            .any(|rt| rt.turn.content.contains("orchestrator.rs")));
    }

    #[tokio::test]
    async fn test_retrieve_token_budget() {
        let store = GraphStore::in_memory().await.unwrap();
        let indexer = TurnIndexer::new(store.clone());

        let messages = vec![
            Message::user("Fix orchestrator.rs"),
            Message::assistant("Fixed orchestrator.rs"),
            Message::user("Also fix orchestrator.rs error handling"),
            Message::assistant("Done with orchestrator.rs"),
        ];
        indexer.index_session("s1", &messages).await.unwrap();

        let retriever = GraphRagRetriever::new(store);
        // Very tight token budget
        let results = retriever.retrieve("orchestrator.rs", 10, 5).await.unwrap();
        assert!(results.len() <= 2); // budget limits results
    }

    #[tokio::test]
    async fn test_retrieve_empty() {
        let store = GraphStore::in_memory().await.unwrap();
        let retriever = GraphRagRetriever::new(store);
        let results = retriever.retrieve("something", 10, 10000).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_chronological_ordering() {
        let store = GraphStore::in_memory().await.unwrap();
        let indexer = TurnIndexer::new(store.clone());

        let messages = vec![
            Message::user("First mention of orchestrator.rs"),
            Message::assistant("Working on orchestrator.rs"),
            Message::user("Second fix for orchestrator.rs"),
        ];
        indexer.index_session("s1", &messages).await.unwrap();

        let retriever = GraphRagRetriever::new(store);
        let results = retriever
            .retrieve("orchestrator.rs", 10, 10000)
            .await
            .unwrap();

        // Should be in chronological order
        for w in results.windows(2) {
            assert!(w[0].turn.turn_index <= w[1].turn.turn_index);
        }
    }
}
