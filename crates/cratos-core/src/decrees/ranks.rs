//! Rank System (Ranks) structure

use serde::{Deserialize, Serialize};

/// Rank system complete structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ranks {
    /// Metadata
    pub meta: RanksMeta,
    /// Rank list
    pub ranks: Vec<Rank>,
    /// Promotion rules
    pub promotion: PromotionRules,
}

/// Rank system metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RanksMeta {
    /// Title
    pub title: String,
    /// Motto
    pub motto: String,
}

/// Individual rank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rank {
    /// Level range (e.g., "1-2", "3", "255")
    pub level: RankLevel,
    /// Rank title (English)
    pub title: String,
    /// Rank title (Korean)
    pub title_kr: String,
    /// Leadership requirements
    pub requirements: Vec<String>,
    /// Permission list
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Level range
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RankLevel {
    /// Single level (e.g., 3, 255)
    Single(u8),
    /// Level range (e.g., [1, 2])
    Range {
        /// Minimum level
        min: u8,
        /// Maximum level
        max: u8,
    },
}

impl RankLevel {
    /// Check if level is within range
    #[must_use]
    pub fn contains(&self, level: u8) -> bool {
        match self {
            Self::Single(l) => *l == level,
            Self::Range { min, max } => level >= *min && level <= *max,
        }
    }

    /// Display string
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Single(255) => "∞".to_string(),
            Self::Single(l) => format!("Lv{l}"),
            Self::Range { min, max } => format!("Lv{min}~{max}"),
        }
    }
}

/// Promotion rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionRules {
    /// Base formula description
    pub formula: String,
    /// Level-specific additional requirements
    #[serde(default)]
    pub additional: Vec<AdditionalRequirement>,
}

/// Additional promotion requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalRequirement {
    /// Applicable level (e.g., "3+", "5+")
    pub level: String,
    /// Requirement description
    pub requirement: String,
}

impl Ranks {
    /// Find rank for given level
    #[must_use]
    pub fn get_rank_for_level(&self, level: u8) -> Option<&Rank> {
        self.ranks.iter().find(|r| r.level.contains(level))
    }

    /// Return rank count
    #[must_use]
    pub fn rank_count(&self) -> usize {
        self.ranks.len()
    }

    /// Validate (minimum 8 ranks: Mortal ~ Supreme)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.ranks.len() >= 8
    }

    /// Generate formatted output
    #[must_use]
    pub fn format_display(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("# {}\n\n", self.meta.title));
        output.push_str(&format!("> {}\n\n", self.meta.motto));
        output.push_str("---\n\n");

        output.push_str("## Rank Structure\n\n");
        output.push_str("| Level | Title | Korean | Requirements |\n");
        output.push_str("|-------|-------|--------|---------------|\n");

        for rank in &self.ranks {
            let reqs = rank.requirements.join(", ");
            output.push_str(&format!(
                "| {} | **{}** | {} | {} |\n",
                rank.level.display(),
                rank.title,
                rank.title_kr,
                reqs
            ));
        }

        output.push_str(&format!(
            "\n## Promotion Conditions\n\n{}\n",
            self.promotion.formula
        ));

        output
    }
}

#[cfg(test)]
mod tests;

