//! Skill router for matching user input to skills.
//!
//! The router determines which skill (if any) should handle a user request
//! based on keywords, regex patterns, and intent classification.
//!
//! # Overview
//!
//! The [`SkillRouter`] uses a multi-factor scoring system:
//!
//! | Factor | Weight | Description |
//! |--------|--------|-------------|
//! | Keywords | 0.4 | Exact word matches from trigger keywords |
//! | Regex | 0.5 | Pattern matches with named capture groups |
//! | Intent | 0.6 | Intent classification (e.g., "file_operation") |
//! | Priority | 0.1 | Skill priority bonus |
//!
//! # Scoring Algorithm
//!
//! 1. Calculate individual scores for keyword, regex, and intent matching
//! 2. Apply weights to each score
//! 3. Add priority bonus
//! 4. Normalize to 0-1 range
//! 5. Filter by minimum score threshold
//! 6. Sort by score (descending), then priority
//!
//! # Example
//!
//! ```ignore
//! use cratos_skills::{SkillRouter, SkillRegistry, RouterConfig};
//!
//! let config = RouterConfig {
//!     min_score: 0.4,           // Higher threshold for precision
//!     keyword_weight: 0.5,       // Emphasize keyword matches
//!     max_input_length: 5_000,   // Stricter input limit
//!     ..Default::default()
//! };
//!
//! let mut router = SkillRouter::with_config(registry, config);
//!
//! // Get all matches
//! let results = router.route("read the config file").await;
//! for r in &results {
//!     println!("{}: {:.2} ({:?})", r.skill.name, r.score, r.match_reason);
//! }
//!
//! // Get best match only
//! if let Some(best) = router.route_best("read the config file").await {
//!     println!("Best match: {}", best.skill.name);
//! }
//! ```
//!
//! # Regex Capture Groups
//!
//! Named capture groups are extracted and available in `captured_groups`:
//!
//! ```text
//! Trigger regex: r"read\s+(?P<path>\S+)"
//! Input: "read /etc/config"
//! Result: captured_groups = {"path": "/etc/config"}
//! ```
//!
//! # Security
//!
//! - **Input length limit**: Prevents DoS via large inputs
//! - **Pattern length limit**: Prevents ReDoS via complex patterns
//! - **Compiled pattern cache**: Avoids repeated regex compilation

use crate::registry::SkillRegistry;
use crate::skill::Skill;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use tracing::{debug, instrument};

/// Reason why a skill was matched
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchReason {
    /// Matched via keyword
    Keyword(String),
    /// Matched via regex pattern
    Regex(String),
    /// Matched via intent classification
    Intent(String),
    /// Combined match (multiple reasons)
    Combined,
}

/// Result of routing a user input to a skill
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// The matched skill
    pub skill: std::sync::Arc<Skill>,
    /// Match score (0.0 - 1.0)
    pub score: f32,
    /// Reason for the match
    pub match_reason: MatchReason,
    /// Matched keywords (if any)
    pub matched_keywords: Vec<String>,
    /// Captured groups from regex (if any)
    pub captured_groups: HashMap<String, String>,
}

/// Configuration for the skill router
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Minimum score to consider a match
    pub min_score: f32,
    /// Weight for keyword matches
    pub keyword_weight: f32,
    /// Weight for regex matches
    pub regex_weight: f32,
    /// Weight for intent matches
    pub intent_weight: f32,
    /// Bonus for skill priority
    pub priority_bonus: f32,
    /// Maximum input length (security: DoS prevention)
    pub max_input_length: usize,
    /// Maximum regex pattern length (security: ReDoS prevention)
    pub max_pattern_length: usize,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            min_score: 0.3,
            keyword_weight: 0.4,
            regex_weight: 0.5,
            intent_weight: 0.6,
            priority_bonus: 0.1,
            max_input_length: 10_000, // 10KB max input
            max_pattern_length: 500,  // 500 chars max pattern
        }
    }
}

/// Skill router for matching inputs to skills
pub struct SkillRouter {
    registry: SkillRegistry,
    config: RouterConfig,
    /// Compiled regex patterns (cached)
    compiled_patterns: HashMap<String, Regex>,
}

impl SkillRouter {
    /// Create a new router with a registry
    pub fn new(registry: SkillRegistry) -> Self {
        Self {
            registry,
            config: RouterConfig::default(),
            compiled_patterns: HashMap::new(),
        }
    }

    /// Create a router with custom configuration
    pub fn with_config(registry: SkillRegistry, config: RouterConfig) -> Self {
        Self {
            registry,
            config,
            compiled_patterns: HashMap::new(),
        }
    }

    /// Route an input text to matching skills
    #[instrument(skip(self), fields(input_len = input_text.len()))]
    pub async fn route(&mut self, input_text: &str) -> Vec<RoutingResult> {
        // SECURITY: Prevent DoS via extremely long inputs
        if input_text.len() > self.config.max_input_length {
            debug!(
                "Input too long ({} > {}), skipping routing",
                input_text.len(),
                self.config.max_input_length
            );
            return Vec::new();
        }

        let input_lower = input_text.to_lowercase();
        let input_words: Vec<&str> = input_lower.split_whitespace().collect();

        let active_skills = self.registry.get_active().await;
        let mut results = Vec::new();

        for skill in active_skills {
            if let Some(result) = self.match_skill(&skill, input_text, &input_lower, &input_words) {
                results.push(result);
            }
        }

        // Sort by score (descending), then by priority (descending)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.skill.trigger.priority.cmp(&a.skill.trigger.priority))
        });

        debug!(
            "Routed input to {} matching skills (top score: {:.2})",
            results.len(),
            results.first().map(|r| r.score).unwrap_or(0.0)
        );

        results
    }

    /// Get the best matching skill (if score is above threshold)
    #[instrument(skip(self))]
    pub async fn route_best(&mut self, input_text: &str) -> Option<RoutingResult> {
        let results = self.route(input_text).await;
        results
            .into_iter()
            .find(|r| r.score >= self.config.min_score)
    }

    /// Check if a specific skill matches the input
    fn match_skill(
        &mut self,
        skill: &std::sync::Arc<Skill>,
        input_text: &str,
        input_lower: &str,
        input_words: &[&str],
    ) -> Option<RoutingResult> {
        let mut total_score = 0.0;
        let mut matched_keywords = Vec::new();
        let mut captured_groups = HashMap::new();
        let mut match_reasons = Vec::new();

        // Keyword matching
        let keyword_score = self.match_keywords(skill, input_words, &mut matched_keywords);
        if keyword_score > 0.0 {
            total_score += keyword_score * self.config.keyword_weight;
            if let Some(kw) = matched_keywords.first() {
                match_reasons.push(MatchReason::Keyword(kw.clone()));
            }
        }

        // Regex matching
        let (regex_score, regex_captures) = self.match_regex(skill, input_text);
        if regex_score > 0.0 {
            total_score += regex_score * self.config.regex_weight;
            captured_groups = regex_captures;
            if let Some(pattern) = skill.trigger.regex_patterns.first() {
                match_reasons.push(MatchReason::Regex(pattern.clone()));
            }
        }

        // Intent matching (simple for now - could be LLM-based)
        let intent_score = self.match_intents(skill, input_lower);
        if intent_score > 0.0 {
            total_score += intent_score * self.config.intent_weight;
            if let Some(intent) = skill.trigger.intents.first() {
                match_reasons.push(MatchReason::Intent(intent.clone()));
            }
        }

        // Apply priority bonus
        let priority_bonus = (skill.trigger.priority as f32 / 100.0) * self.config.priority_bonus;
        total_score += priority_bonus;

        // Normalize score to 0-1 range
        let max_possible = self.config.keyword_weight
            + self.config.regex_weight
            + self.config.intent_weight
            + self.config.priority_bonus;
        total_score = (total_score / max_possible).min(1.0);

        if total_score > 0.0 {
            let match_reason = match match_reasons.len() {
                0 => return None,
                1 => match_reasons.into_iter().next()?,
                _ => MatchReason::Combined,
            };

            Some(RoutingResult {
                skill: skill.clone(),
                score: total_score,
                match_reason,
                matched_keywords,
                captured_groups,
            })
        } else {
            None
        }
    }

    /// Match keywords against input words
    fn match_keywords(
        &self,
        skill: &Skill,
        input_words: &[&str],
        matched: &mut Vec<String>,
    ) -> f32 {
        if skill.trigger.keywords.is_empty() {
            return 0.0;
        }

        let mut matches = 0;
        for keyword in &skill.trigger.keywords {
            let keyword_lower = keyword.to_lowercase();
            if input_words.iter().any(|w| *w == keyword_lower) {
                matched.push(keyword.clone());
                matches += 1;
            }
        }

        if matches > 0 {
            // Score based on percentage of keywords matched
            matches as f32 / skill.trigger.keywords.len() as f32
        } else {
            0.0
        }
    }

    /// Match regex patterns against input text
    fn match_regex(&mut self, skill: &Skill, input_text: &str) -> (f32, HashMap<String, String>) {
        let mut best_score = 0.0;
        let mut captures = HashMap::new();

        for pattern in &skill.trigger.regex_patterns {
            // SECURITY: Prevent ReDoS via overly complex patterns
            if pattern.len() > self.config.max_pattern_length {
                debug!(
                    "Regex pattern too long ({} > {}), skipping",
                    pattern.len(),
                    self.config.max_pattern_length
                );
                continue;
            }

            // Get or compile the regex
            let regex = if let Some(r) = self.compiled_patterns.get(pattern) {
                r
            } else {
                match Regex::new(pattern) {
                    Ok(r) => self.compiled_patterns.entry(pattern.clone()).or_insert(r),
                    Err(e) => {
                        debug!("Invalid regex pattern '{}': {}", pattern, e);
                        continue;
                    }
                }
            };

            if let Some(caps) = regex.captures(input_text) {
                best_score = 1.0;

                // Extract named captures
                for name in regex.capture_names().flatten() {
                    if let Some(m) = caps.name(name) {
                        captures.insert(name.to_string(), m.as_str().to_string());
                    }
                }

                break; // Use first matching pattern
            }
        }

        (best_score, captures)
    }

    /// Match intents against input (simple keyword-based for now)
    fn match_intents(&self, skill: &Skill, input_lower: &str) -> f32 {
        if skill.trigger.intents.is_empty() {
            return 0.0;
        }

        // Simple intent matching based on common words
        // In production, this would use an LLM or classifier
        let intent_keywords: HashMap<&str, Vec<&str>> = [
            (
                "file_operation",
                vec!["file", "read", "write", "create", "delete"],
            ),
            (
                "git_operation",
                vec!["git", "commit", "push", "pull", "branch"],
            ),
            (
                "code_generation",
                vec!["generate", "create", "write", "code"],
            ),
            ("search", vec!["find", "search", "look", "where"]),
            ("explain", vec!["explain", "what", "how", "why"]),
        ]
        .into_iter()
        .collect();

        let mut best_score: f32 = 0.0;
        for intent in &skill.trigger.intents {
            if let Some(keywords) = intent_keywords.get(intent.as_str()) {
                let matches = keywords
                    .iter()
                    .filter(|kw| input_lower.contains(**kw))
                    .count();
                let score = matches as f32 / keywords.len() as f32;
                best_score = best_score.max(score);
            }
        }

        best_score
    }

    /// Clear the compiled regex cache
    pub fn clear_cache(&mut self) {
        self.compiled_patterns.clear();
    }

    /// Route an input text to matching skills with persona-specific bonuses.
    ///
    /// Skills where the persona has high proficiency (success_rate >= threshold) get a bonus.
    /// This makes personas more likely to use skills they're good at.
    ///
    /// # Arguments
    ///
    /// * `input_text` - The user input to match against skills
    /// * `persona_skill_proficiency` - Map of skill_name -> success_rate for the persona
    /// * `bonus` - Score bonus to add for proficient skills (default: 0.2)
    /// * `threshold` - Minimum success rate to be considered proficient (default: 0.7)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let proficiency = persona_store.get_skill_proficiency_map("sindri").await?;
    /// let results = router.route_for_persona(
    ///     "build the API",
    ///     &proficiency,
    ///     0.2,  // bonus
    ///     0.7,  // threshold
    /// ).await;
    /// ```
    #[instrument(skip(self, persona_skill_proficiency), fields(input_len = input_text.len()))]
    pub async fn route_for_persona(
        &mut self,
        input_text: &str,
        persona_skill_proficiency: &std::collections::HashMap<String, f64>,
        bonus: f32,
        threshold: f64,
    ) -> Vec<RoutingResult> {
        // Get base routing results
        let mut results = self.route(input_text).await;

        // Apply persona skill bonus
        for result in &mut results {
            if let Some(&success_rate) = persona_skill_proficiency.get(&result.skill.name) {
                if success_rate >= threshold {
                    let old_score = result.score;
                    result.score = (result.score + bonus).min(1.0);
                    debug!(
                        skill = %result.skill.name,
                        old_score = %old_score,
                        new_score = %result.score,
                        success_rate = %success_rate,
                        "Applied persona skill bonus"
                    );
                }
            }
        }

        // Re-sort after applying bonuses
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.skill.trigger.priority.cmp(&a.skill.trigger.priority))
        });

        results
    }

    /// Route and get the best match with persona-specific bonuses
    #[instrument(skip(self, persona_skill_proficiency))]
    pub async fn route_best_for_persona(
        &mut self,
        input_text: &str,
        persona_skill_proficiency: &std::collections::HashMap<String, f64>,
        bonus: f32,
        threshold: f64,
    ) -> Option<RoutingResult> {
        let results = self
            .route_for_persona(input_text, persona_skill_proficiency, bonus, threshold)
            .await;
        results
            .into_iter()
            .find(|r| r.score >= self.config.min_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{SkillCategory, SkillTrigger};

    async fn create_test_router() -> SkillRouter {
        let registry = SkillRegistry::new();

        // Add a test skill
        let mut skill = Skill::new("file_reader", "Read files", SkillCategory::Custom)
            .with_trigger(
                SkillTrigger::with_keywords(vec!["read".to_string(), "file".to_string()])
                    .add_pattern(r"read\s+(?P<path>\S+)")
                    .add_intent("file_operation")
                    .with_priority(10),
            );
        skill.activate();
        registry.register(skill).await.unwrap();

        // Add another skill
        let mut skill2 = Skill::new("git_committer", "Git commit", SkillCategory::Custom)
            .with_trigger(
                SkillTrigger::with_keywords(vec!["commit".to_string(), "git".to_string()])
                    .with_priority(5),
            );
        skill2.activate();
        registry.register(skill2).await.unwrap();

        SkillRouter::new(registry)
    }

    #[tokio::test]
    async fn test_keyword_routing() {
        let mut router = create_test_router().await;

        let results = router.route("please read the file").await;
        assert!(!results.is_empty());
        assert_eq!(results[0].skill.name, "file_reader");
        assert!(results[0].matched_keywords.contains(&"read".to_string()));
    }

    #[tokio::test]
    async fn test_regex_routing() {
        let mut router = create_test_router().await;

        let results = router.route("read /path/to/file.txt").await;
        assert!(!results.is_empty());
        assert_eq!(results[0].skill.name, "file_reader");
        assert_eq!(
            results[0].captured_groups.get("path"),
            Some(&"/path/to/file.txt".to_string())
        );
    }

    #[tokio::test]
    async fn test_best_match() {
        let mut router = create_test_router().await;

        let best = router.route_best("read the file please").await;
        assert!(best.is_some());
        assert_eq!(best.unwrap().skill.name, "file_reader");
    }

    #[tokio::test]
    async fn test_no_match() {
        let mut router = create_test_router().await;

        let _results = router.route("do something completely different").await;
        // May have some low-scoring matches, but best should be None
        let best = router.route_best("do something completely different").await;
        assert!(best.is_none());
    }

    #[tokio::test]
    async fn test_priority_affects_order() {
        let mut router = create_test_router().await;

        // Both skills match "read file commit" but file_reader has higher priority
        let results = router.route("read file and commit").await;
        assert!(results.len() >= 2);
        // Higher priority skill should come first if scores are similar
        let file_reader_pos = results.iter().position(|r| r.skill.name == "file_reader");
        let committer_pos = results.iter().position(|r| r.skill.name == "git_committer");

        // With more keyword matches for file_reader, it should rank higher
        if let (Some(fp), Some(cp)) = (file_reader_pos, committer_pos) {
            // file_reader matches "read" and "file", while git_committer only matches "commit"
            // file_reader should be ranked higher
            assert!(results[fp].score >= results[cp].score || fp < cp);
        }
    }

    #[tokio::test]
    async fn test_case_insensitive() {
        let mut router = create_test_router().await;

        let results_lower = router.route("read file").await;
        let results_upper = router.route("READ FILE").await;

        assert!(!results_lower.is_empty());
        assert!(!results_upper.is_empty());
        assert_eq!(results_lower[0].skill.name, results_upper[0].skill.name);
    }

    #[test]
    fn test_match_reason_serialization() {
        let reason = MatchReason::Keyword("test".to_string());
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("keyword"));
    }

    #[tokio::test]
    async fn test_security_input_too_long() {
        let registry = SkillRegistry::new();
        let config = RouterConfig {
            max_input_length: 100,
            ..Default::default()
        };
        let mut router = SkillRouter::with_config(registry, config);

        // Input exceeds max length
        let long_input = "a".repeat(200);
        let results = router.route(&long_input).await;

        // Should return empty results (rejected for security)
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_security_regex_pattern_too_long() {
        let registry = SkillRegistry::new();

        // Create skill with overly long regex pattern
        let long_pattern = format!("{}+", "a".repeat(600));
        let mut skill = Skill::new("test", "Test", SkillCategory::Custom)
            .with_trigger(SkillTrigger::with_keywords(vec![]).add_pattern(&long_pattern));
        skill.activate();
        registry.register(skill).await.unwrap();

        let config = RouterConfig {
            max_pattern_length: 500,
            ..Default::default()
        };
        let mut router = SkillRouter::with_config(registry, config);

        // Pattern should be skipped due to length
        let results = router.route("aaaaaa").await;
        assert!(results.is_empty());
    }
}
