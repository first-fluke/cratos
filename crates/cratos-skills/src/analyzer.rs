//! Pattern analyzer for detecting frequent tool usage patterns.
//!
//! This module analyzes execution history from cratos-replay to detect
//! patterns that can be converted into auto-generated skills.
//!
//! # Overview
//!
//! The [`PatternAnalyzer`] examines user execution history to find recurring
//! tool sequences. When a sequence appears multiple times with sufficient
//! confidence, it becomes a candidate for skill generation.
//!
//! # Pattern Detection Algorithm
//!
//! 1. **Event Collection**: Query recent executions within the analysis window
//! 2. **Sequence Extraction**: Extract tool call order per execution
//! 3. **N-gram Analysis**: Calculate frequency of 2 to N tool combinations
//! 4. **Confidence Calculation**: `occurrence_count / total_executions`
//! 5. **Keyword Extraction**: Extract user input keywords (stopwords removed)
//! 6. **Pattern Ranking**: Sort by confidence and occurrence count
//!
//! # Example
//!
//! ```ignore
//! use cratos_skills::{PatternAnalyzer, AnalyzerConfig};
//!
//! // Use custom settings for higher precision
//! let config = AnalyzerConfig {
//!     min_occurrences: 5,       // Require 5+ repetitions
//!     min_confidence: 0.7,       // 70%+ confidence
//!     max_sequence_length: 4,    // Analyze up to 4-tool sequences
//!     analysis_window_days: 14,  // Last 2 weeks only
//! };
//!
//! let analyzer = PatternAnalyzer::with_config(config);
//! let patterns = analyzer.detect_patterns(&event_store).await?;
//!
//! for pattern in &patterns {
//!     if pattern.confidence_score >= 0.8 {
//!         println!("High-confidence pattern: {:?}", pattern.tool_sequence);
//!     }
//! }
//! ```

use crate::error::Result;
use chrono::{DateTime, Duration, Utc};
use cratos_replay::{Event, EventStore, EventType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// Run automated pattern analysis on the default databases.
///
/// This is a convenience function for the orchestrator and scheduler.
/// It opens the default EventStore and SkillStore, runs detection, and saves new patterns.
pub async fn run_auto_analysis(dry_run: bool) -> Result<String> {
    use crate::SkillStore;
    use cratos_replay::{default_db_path, EventStore};

    let replay_path = default_db_path();
    let skill_path = crate::default_skill_db_path();

    if !replay_path.exists() {
        return Ok("Replay database not found, skipping analysis.".to_string());
    }

    let event_store = EventStore::from_path(&replay_path).await?;
    let skill_store = SkillStore::from_path(&skill_path).await?;
    let analyzer = PatternAnalyzer::default();

    let patterns = analyzer.detect_patterns(&event_store).await?;
    let count = patterns.len();

    if count == 0 {
        return Ok("No recurring patterns detected.".to_string());
    }

    if dry_run {
        return Ok(format!("Detected {} potential patterns (dry run).", count));
    }

    let mut saved = 0;
    for p in patterns {
        if skill_store.save_pattern(&p).await.is_ok() {
            saved += 1;
        }
    }

    Ok(format!(
        "Analysis complete: Detected {} patterns, Saved new: {}",
        count, saved
    ))
}

/// Configuration for the pattern analyzer
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Minimum number of occurrences to consider a pattern
    pub min_occurrences: u32,
    /// Minimum confidence score (0.0 - 1.0)
    pub min_confidence: f32,
    /// Maximum sequence length to analyze
    pub max_sequence_length: usize,
    /// Time window for analysis (in days)
    pub analysis_window_days: i64,
    /// Languages for stop word removal (e.g., "en", "ko")
    pub languages: Vec<String>,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            min_occurrences: 3,
            min_confidence: 0.6,
            max_sequence_length: 5,
            analysis_window_days: 30,
            languages: vec!["en".to_string(), "ko".to_string()],
        }
    }
}

/// A detected usage pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    /// Unique identifier
    pub id: Uuid,
    /// Sequence of tool names
    pub tool_sequence: Vec<String>,
    /// Number of times this pattern occurred
    pub occurrence_count: u32,
    /// Confidence score (0.0 - 1.0)
    pub confidence_score: f32,
    /// Keywords extracted from user inputs
    pub extracted_keywords: Vec<String>,
    /// Sample input texts that triggered this pattern
    pub sample_inputs: Vec<String>,
    /// Pattern status
    pub status: PatternStatus,
    /// Associated skill ID (if converted)
    pub converted_skill_id: Option<Uuid>,
    /// When the pattern was detected
    pub detected_at: DateTime<Utc>,
}

/// Status of a detected pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternStatus {
    /// Newly detected
    Detected,
    /// Converted to a skill
    Converted,
    /// Rejected by user
    Rejected,
    /// Expired (too old)
    Expired,
}

impl PatternStatus {
    /// Returns the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Detected => "detected",
            Self::Converted => "converted",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }
}

impl std::str::FromStr for PatternStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "detected" => Ok(Self::Detected),
            "converted" => Ok(Self::Converted),
            "rejected" => Ok(Self::Rejected),
            "expired" => Ok(Self::Expired),
            _ => Err(format!("unknown pattern status: {s}")),
        }
    }
}

/// Pattern analyzer for detecting tool usage patterns
pub struct PatternAnalyzer {
    config: AnalyzerConfig,
    stop_words: std::collections::HashSet<String>,
}

impl Default for PatternAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternAnalyzer {
    /// Create a new pattern analyzer with default configuration
    pub fn new() -> Self {
        Self::with_config(AnalyzerConfig::default())
    }

    /// Create a pattern analyzer with custom configuration
    pub fn with_config(config: AnalyzerConfig) -> Self {
        let mut stop_words = std::collections::HashSet::new();
        
        for lang in &config.languages {
            let words = match lang.as_str() {
                "en" => stop_words::get(stop_words::LANGUAGE::English).iter().map(|s| s.to_string()).collect(),
                "ko" => stop_words::get(stop_words::LANGUAGE::Korean).iter().map(|s| s.to_string()).collect(),
                // Add more as needed or handle dynamically if library supports string matching well
                _ => Vec::new(),
            };
            for word in words {
                stop_words.insert(word);
            }
        }

        Self { config, stop_words }
    }

    /// Extract tool sequences from events
    ///
    /// Returns a list of tool name sequences, one per execution.
    #[instrument(skip(self, events))]
    pub fn extract_tool_sequences(&self, events: &[Event]) -> Vec<Vec<String>> {
        // Group events by execution
        let mut executions: HashMap<Uuid, Vec<&Event>> = HashMap::new();
        for event in events {
            executions
                .entry(event.execution_id)
                .or_default()
                .push(event);
        }

        // Extract tool sequences
        let mut sequences = Vec::new();
        for (_, exec_events) in executions {
            let mut sorted_events: Vec<_> = exec_events.into_iter().collect();
            sorted_events.sort_by_key(|e| e.sequence_num);

            let tools: Vec<String> = sorted_events
                .iter()
                .filter(|e| e.event_type == EventType::ToolCall)
                .filter_map(|e| {
                    e.payload
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .collect();

            if !tools.is_empty() {
                sequences.push(tools);
            }
        }

        debug!("Extracted {} tool sequences", sequences.len());
        sequences
    }

    /// Find frequent n-grams in tool sequences
    ///
    /// Returns a map from n-gram to occurrence count.
    #[instrument(skip(self, sequences))]
    pub fn find_frequent_ngrams(
        &self,
        sequences: &[Vec<String>],
        n: usize,
    ) -> HashMap<Vec<String>, u32> {
        let mut ngram_counts: HashMap<Vec<String>, u32> = HashMap::new();

        for sequence in sequences {
            if sequence.len() < n {
                continue;
            }

            // Extract all n-grams from this sequence
            for window in sequence.windows(n) {
                let ngram: Vec<String> = window.to_vec();
                *ngram_counts.entry(ngram).or_default() += 1;
            }
        }

        // Filter by minimum occurrences
        ngram_counts.retain(|_, count| *count >= self.config.min_occurrences);

        debug!("Found {} frequent {}-grams", ngram_counts.len(), n);
        ngram_counts
    }

    /// Extract keywords from user input events
    #[instrument(skip(self, events))]
    pub fn extract_keywords(&self, events: &[Event]) -> HashMap<Uuid, Vec<String>> {
        let mut keywords_by_execution: HashMap<Uuid, Vec<String>> = HashMap::new();

        for event in events {
            if event.event_type != EventType::UserInput {
                continue;
            }

            if let Some(text) = event.payload.get("text").and_then(|v| v.as_str()) {
                let words: Vec<String> = text
                    .split_whitespace()
                    .filter(|w| w.len() > 2) // Skip short words
                    .filter(|w| !self.is_stop_word(w))
                    .map(|w| w.to_lowercase())
                    .collect();

                keywords_by_execution.insert(event.execution_id, words);
            }
        }

        keywords_by_execution
    }

    /// Detect patterns from the event store
    #[instrument(skip(self, store))]
    pub async fn detect_patterns(&self, store: &EventStore) -> Result<Vec<DetectedPattern>> {
        let window_start = Utc::now() - Duration::days(self.config.analysis_window_days);

        // Get recent executions
        let executions = store.list_recent_executions(1000).await?;
        let recent_executions: Vec<_> = executions
            .into_iter()
            .filter(|e| e.created_at > window_start)
            .collect();

        if recent_executions.is_empty() {
            info!("No recent executions to analyze");
            return Ok(Vec::new());
        }

        // Collect all events from recent executions
        let mut all_events = Vec::new();
        for execution in &recent_executions {
            let events = store.get_execution_events(execution.id).await?;
            all_events.extend(events);
        }

        info!(
            "Analyzing {} events from {} executions",
            all_events.len(),
            recent_executions.len()
        );

        // Extract tool sequences
        let sequences = self.extract_tool_sequences(&all_events);
        if sequences.is_empty() {
            return Ok(Vec::new());
        }

        // Extract keywords
        let keywords_by_execution = self.extract_keywords(&all_events);

        // Find patterns of different lengths
        let mut patterns = Vec::new();
        for n in 2..=self.config.max_sequence_length {
            let ngrams = self.find_frequent_ngrams(&sequences, n);

            for (ngram, count) in ngrams {
                // Calculate confidence based on occurrence rate
                let confidence = count as f32 / sequences.len() as f32;

                if confidence >= self.config.min_confidence {
                    // Find keywords associated with this pattern
                    let associated_keywords =
                        self.find_associated_keywords(&ngram, &all_events, &keywords_by_execution);

                    // Find sample inputs
                    let sample_inputs = self.find_sample_inputs(
                        &ngram,
                        &all_events,
                        5, // Max 5 samples
                    );

                    patterns.push(DetectedPattern {
                        id: Uuid::new_v4(),
                        tool_sequence: ngram,
                        occurrence_count: count,
                        confidence_score: confidence,
                        extracted_keywords: associated_keywords,
                        sample_inputs,
                        status: PatternStatus::Detected,
                        converted_skill_id: None,
                        detected_at: Utc::now(),
                    });
                }
            }
        }

        // Sort by confidence and occurrence count
        patterns.sort_by(|a, b| {
            b.confidence_score
                .partial_cmp(&a.confidence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.occurrence_count.cmp(&a.occurrence_count))
        });

        info!("Detected {} patterns", patterns.len());
        Ok(patterns)
    }

    /// Find keywords associated with a specific tool sequence
    fn find_associated_keywords(
        &self,
        tool_sequence: &[String],
        events: &[Event],
        keywords_by_execution: &HashMap<Uuid, Vec<String>>,
    ) -> Vec<String> {
        // Find executions that contain this tool sequence
        let mut matching_executions = Vec::new();

        let mut executions: HashMap<Uuid, Vec<&Event>> = HashMap::new();
        for event in events {
            executions
                .entry(event.execution_id)
                .or_default()
                .push(event);
        }

        for (exec_id, exec_events) in executions {
            let tools: Vec<String> = exec_events
                .iter()
                .filter(|e| e.event_type == EventType::ToolCall)
                .filter_map(|e| {
                    e.payload
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .collect();

            if contains_subsequence(&tools, tool_sequence) {
                matching_executions.push(exec_id);
            }
        }

        // Count keyword occurrences across matching executions
        let mut keyword_counts: HashMap<String, u32> = HashMap::new();
        for exec_id in matching_executions {
            if let Some(keywords) = keywords_by_execution.get(&exec_id) {
                for keyword in keywords {
                    *keyword_counts.entry(keyword.clone()).or_default() += 1;
                }
            }
        }

        // Return top keywords
        let mut keywords: Vec<_> = keyword_counts.into_iter().collect();
        keywords.sort_by(|a, b| b.1.cmp(&a.1));
        keywords.into_iter().take(10).map(|(k, _)| k).collect()
    }

    /// Check if a word is a stop word
    fn is_stop_word(&self, word: &str) -> bool {
        self.stop_words.contains(word) || self.stop_words.contains(&word.to_lowercase())
    }

    /// Find sample user inputs that led to a tool sequence
    fn find_sample_inputs(
        &self,
        tool_sequence: &[String],
        events: &[Event],
        max_samples: usize,
    ) -> Vec<String> {
        let mut samples = Vec::new();

        let mut executions: HashMap<Uuid, Vec<&Event>> = HashMap::new();
        for event in events {
            executions
                .entry(event.execution_id)
                .or_default()
                .push(event);
        }

        for (_, exec_events) in executions {
            if samples.len() >= max_samples {
                break;
            }

            let tools: Vec<String> = exec_events
                .iter()
                .filter(|e| e.event_type == EventType::ToolCall)
                .filter_map(|e| {
                    e.payload
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .collect();

            if contains_subsequence(&tools, tool_sequence) {
                // Find user input for this execution
                if let Some(input_event) = exec_events
                    .iter()
                    .find(|e| e.event_type == EventType::UserInput)
                {
                    if let Some(text) = input_event.payload.get("text").and_then(|v| v.as_str()) {
                        samples.push(text.to_string());
                    }
                }
            }
        }

        samples
    }
}

/// Check if a sequence contains a subsequence
fn contains_subsequence<T: PartialEq>(sequence: &[T], subsequence: &[T]) -> bool {
    if subsequence.is_empty() {
        return true;
    }
    if subsequence.len() > sequence.len() {
        return false;
    }

    sequence
        .windows(subsequence.len())
        .any(|w| w == subsequence)
}

/// Check if a word is a stop word


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_subsequence() {
        let seq = vec!["a", "b", "c", "d", "e"];
        assert!(contains_subsequence(&seq, &["b", "c", "d"]));
        assert!(contains_subsequence(&seq, &["a"]));
        assert!(contains_subsequence(&seq, &["e"]));
        assert!(contains_subsequence(&seq, &[]));
        assert!(!contains_subsequence(&seq, &["a", "c"]));
        assert!(!contains_subsequence(&seq, &["f"]));
    }

    #[test]
    fn test_is_stop_word() {
        let analyzer = PatternAnalyzer::default();
        assert!(analyzer.is_stop_word("the"));
        assert!(analyzer.is_stop_word("The"));
        assert!(analyzer.is_stop_word("a"));
        assert!(!analyzer.is_stop_word("file"));
        assert!(!analyzer.is_stop_word("read"));
    }

    #[test]
    fn test_find_frequent_ngrams() {
        let analyzer = PatternAnalyzer::with_config(AnalyzerConfig {
            min_occurrences: 2,
            ..Default::default()
        });

        let sequences = vec![
            vec!["file_read".to_string(), "git_commit".to_string()],
            vec!["file_read".to_string(), "git_commit".to_string()],
            vec!["file_read".to_string(), "git_commit".to_string()],
            vec!["exec".to_string()],
        ];

        let ngrams = analyzer.find_frequent_ngrams(&sequences, 2);

        assert_eq!(ngrams.len(), 1);
        assert_eq!(
            ngrams.get(&vec!["file_read".to_string(), "git_commit".to_string()]),
            Some(&3)
        );
    }

    #[test]
    fn test_analyzer_config_default() {
        let config = AnalyzerConfig::default();
        assert_eq!(config.min_occurrences, 3);
        assert_eq!(config.min_confidence, 0.6);
        assert_eq!(config.max_sequence_length, 5);
        assert_eq!(config.analysis_window_days, 30);
    }

    #[test]
    fn test_pattern_status() {
        assert_eq!(PatternStatus::Detected.as_str(), "detected");
        assert_eq!(PatternStatus::Converted.as_str(), "converted");

        let parsed: PatternStatus = "detected".parse().unwrap();
        assert_eq!(parsed, PatternStatus::Detected);
    }
}
