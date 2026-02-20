//! Achievement Record Data Structures
//!
//! Chronicle, Quest, Judgment and other record-related type definitions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Chronicle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChronicleStatus {
    /// Active state
    Active,
    /// Inactive state
    Inactive,
    /// Promoted
    Promoted,
    /// Silence punishment (Laws Article 8)
    Silenced,
}

impl Default for ChronicleStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// Quest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    /// Quest description
    pub description: String,
    /// Completion status
    pub completed: bool,
    /// Completion timestamp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

impl Quest {
    /// Create new quest
    #[must_use]
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            completed: false,
            completed_at: None,
        }
    }

    /// Mark quest as complete
    pub fn complete(&mut self) {
        self.completed = true;
        self.completed_at = Some(Utc::now());
    }
}

/// Chronicle entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronicleEntry {
    /// Entry timestamp
    pub timestamp: DateTime<Utc>,
    /// Achievement/work content
    pub achievement: String,
    /// Law reference (e.g., "2" → Laws Article 2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law_reference: Option<String>,
    /// Related commit hash
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
}

impl ChronicleEntry {
    /// Create new chronicle entry
    #[must_use]
    pub fn new(achievement: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            achievement: achievement.into(),
            law_reference: None,
            commit_hash: None,
        }
    }

    /// Add law reference
    #[must_use]
    pub fn with_law(mut self, law_ref: impl Into<String>) -> Self {
        self.law_reference = Some(law_ref.into());
        self
    }

    /// Add commit hash
    #[must_use]
    pub fn with_commit(mut self, hash: impl Into<String>) -> Self {
        self.commit_hash = Some(hash.into());
        self
    }
}

/// Judgment (Evaluation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Judgment {
    /// Evaluator (persona name or user)
    pub evaluator: String,
    /// Evaluation timestamp
    pub timestamp: DateTime<Utc>,
    /// Evaluation comment
    pub comment: String,
    /// Score (1.0 - 5.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

impl Judgment {
    /// Create new judgment
    #[must_use]
    pub fn new(evaluator: impl Into<String>, comment: impl Into<String>) -> Self {
        Self {
            evaluator: evaluator.into(),
            timestamp: Utc::now(),
            comment: comment.into(),
            score: None,
        }
    }

    /// Add score
    #[must_use]
    pub fn with_score(mut self, score: f32) -> Self {
        self.score = Some(score.clamp(1.0, 5.0));
        self
    }
}

/// Chronicle (Achievement Record)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chronicle {
    /// Persona name
    pub persona_name: String,
    /// Current level
    pub level: u8,
    /// Status
    #[serde(default)]
    pub status: ChronicleStatus,
    /// Objectives list
    #[serde(default)]
    pub objectives: Vec<String>,
    /// Quests list
    #[serde(default)]
    pub quests: Vec<Quest>,
    /// Activity log
    #[serde(default)]
    pub log: Vec<ChronicleEntry>,
    /// Judgments list
    #[serde(default)]
    pub judgments: Vec<Judgment>,
    /// Average rating
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<f32>,
    /// Skill proficiency map (skill_name -> success_rate)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub skill_proficiency: std::collections::HashMap<String, f64>,
    /// Auto-assigned skills (earned through usage patterns)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_assigned_skills: Vec<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Chronicle {
    /// Create new chronicle
    #[must_use]
    pub fn new(persona_name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            persona_name: persona_name.into(),
            level: 1,
            status: ChronicleStatus::Active,
            objectives: Vec::new(),
            quests: Vec::new(),
            log: Vec::new(),
            judgments: Vec::new(),
            rating: None,
            skill_proficiency: std::collections::HashMap::new(),
            auto_assigned_skills: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Update skill proficiency for this persona
    ///
    /// # Arguments
    ///
    /// * `skill_name` - The skill name
    /// * `success_rate` - The success rate (0.0 - 1.0)
    pub fn update_skill_proficiency(&mut self, skill_name: &str, success_rate: f64) {
        self.skill_proficiency
            .insert(skill_name.to_string(), success_rate);
        self.updated_at = Utc::now();
    }

    /// Record that a skill was auto-assigned to this persona
    pub fn record_auto_assignment(&mut self, skill_name: &str) {
        if !self.auto_assigned_skills.contains(&skill_name.to_string()) {
            self.auto_assigned_skills.push(skill_name.to_string());
            self.add_entry(
                &format!("Skill '{}' auto-assigned through mastery", skill_name),
                None,
            );
        }
    }

    /// Get mastered skills (success_rate >= threshold)
    #[must_use]
    pub fn mastered_skills(&self, threshold: f64) -> Vec<(&String, f64)> {
        self.skill_proficiency
            .iter()
            .filter(|(_, &rate)| rate >= threshold)
            .map(|(name, &rate)| (name, rate))
            .collect()
    }

    /// Add chronicle entry
    pub fn add_entry(&mut self, achievement: &str, law_ref: Option<&str>) {
        let mut entry = ChronicleEntry::new(achievement);
        if let Some(law) = law_ref {
            entry = entry.with_law(law);
        }
        self.log.push(entry);
        self.updated_at = Utc::now();
    }

    /// Add entry with commit hash
    pub fn add_entry_with_commit(
        &mut self,
        achievement: &str,
        law_ref: Option<&str>,
        commit: &str,
    ) {
        let mut entry = ChronicleEntry::new(achievement);
        if let Some(law) = law_ref {
            entry = entry.with_law(law);
        }
        entry = entry.with_commit(commit);
        self.log.push(entry);
        self.updated_at = Utc::now();
    }

    /// Add quest
    pub fn add_quest(&mut self, description: &str) {
        self.quests.push(Quest::new(description));
        self.updated_at = Utc::now();
    }

    /// Complete quest by index
    ///
    /// # Returns
    /// `true` on success, `false` if index out of bounds
    pub fn complete_quest(&mut self, index: usize) -> bool {
        if let Some(quest) = self.quests.get_mut(index) {
            quest.complete();
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Add objective
    pub fn add_objective(&mut self, objective: &str) {
        self.objectives.push(objective.to_string());
        self.updated_at = Utc::now();
    }

    /// Add judgment
    pub fn add_judgment(&mut self, evaluator: &str, comment: &str, score: Option<f32>) {
        let mut judgment = Judgment::new(evaluator, comment);
        if let Some(s) = score {
            judgment = judgment.with_score(s);
        }
        self.judgments.push(judgment);
        self.update_rating();
        self.updated_at = Utc::now();
    }

    /// Calculate average rating
    fn update_rating(&mut self) {
        let scores: Vec<f32> = self.judgments.iter().filter_map(|j| j.score).collect();
        if scores.is_empty() {
            self.rating = None;
        } else {
            let sum: f32 = scores.iter().sum();
            self.rating = Some(sum / scores.len() as f32);
        }
    }

    /// Count completed quests
    #[must_use]
    pub fn completed_quests(&self) -> usize {
        self.quests.iter().filter(|q| q.completed).count()
    }

    /// Count pending quests
    #[must_use]
    pub fn pending_quests(&self) -> usize {
        self.quests.iter().filter(|q| !q.completed).count()
    }

    /// Minimum average rating required for promotion (judgment.toml Article 3)
    const PROMOTION_RATING_THRESHOLD: f32 = 3.5;

    /// Check promotion eligibility
    ///
    /// Promotion requires BOTH:
    /// 1. log entries >= (current level + 1) * 5
    /// 2. average rating >= 3.5 (or no judgments yet for fresh personas)
    #[must_use]
    pub fn is_promotion_eligible(&self) -> bool {
        let required = (self.level as usize + 1) * 5;
        let has_enough_entries = self.log.len() >= required;
        let has_good_rating = self
            .rating
            .map(|r| r >= Self::PROMOTION_RATING_THRESHOLD)
            .unwrap_or(true); // no judgments yet → allow promotion
        has_enough_entries && has_good_rating
    }

    /// Return entries needed until promotion
    #[must_use]
    pub fn entries_until_promotion(&self) -> usize {
        let required = (self.level as usize + 1) * 5;
        required.saturating_sub(self.log.len())
    }

    /// Return the rating gap until promotion (0.0 if already sufficient)
    #[must_use]
    pub fn rating_gap(&self) -> f32 {
        match self.rating {
            Some(r) if r < Self::PROMOTION_RATING_THRESHOLD => Self::PROMOTION_RATING_THRESHOLD - r,
            _ => 0.0,
        }
    }

    /// Level up (promote)
    pub fn promote(&mut self) -> bool {
        if self.is_promotion_eligible() && self.level < 10 {
            self.level += 1;
            self.status = ChronicleStatus::Promoted;
            self.add_entry(
                &format!("Promoted to Lv{}", self.level),
                Some("9"), // Laws Article 9: Personnel and Evaluation
            );
            self.status = ChronicleStatus::Active;
            true
        } else {
            false
        }
    }

    /// Apply silence punishment (Laws Article 8)
    pub fn apply_silence(&mut self) {
        self.status = ChronicleStatus::Silenced;
        self.add_entry("Silence punishment applied", Some("8"));
    }

    /// Release silence punishment
    pub fn release_silence(&mut self) {
        if self.status == ChronicleStatus::Silenced {
            self.status = ChronicleStatus::Active;
            self.add_entry("Silence punishment released", None);
        }
    }
}

#[cfg(test)]
mod tests;

