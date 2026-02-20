//! Persona Domain (Role Classification)

use serde::{Deserialize, Serialize};
use std::fmt;

/// Persona Domain - Role Classification
///
/// Core 5 (Phase 1):
/// - Orchestrator: Supreme controller (Cratos)
/// - Pm: Project management (Athena)
/// - Dev: Development (Sindri)
/// - Qa: Quality assurance (Heimdall)
/// - Researcher: Research (Mimir)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Domain {
    /// Supreme controller (Cratos)
    Orchestrator,
    /// Project manager (Athena)
    Pm,
    /// Developer (Sindri)
    Dev,
    /// Quality assurance (Heimdall)
    Qa,
    /// Researcher (Mimir)
    Researcher,
    /// Product Owner (Odin)
    Po,
    /// Human Resources (Hestia)
    Hr,
    /// Business Analyst (Norns)
    Ba,
    /// UX Designer (Apollo)
    Ux,
    /// Customer Support (Freya)
    Cs,
    /// Legal (Tyr)
    Legal,
    /// Marketing (Nike)
    Marketing,
    /// DevOps (Thor)
    #[serde(alias = "DEVOPS")]
    DevOps,
}

impl Domain {
    /// Return string representation
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Orchestrator => "ORCHESTRATOR",
            Self::Pm => "PM",
            Self::Dev => "DEV",
            Self::Qa => "QA",
            Self::Researcher => "RESEARCHER",
            Self::Po => "PO",
            Self::Hr => "HR",
            Self::Ba => "BA",
            Self::Ux => "UX",
            Self::Cs => "CS",
            Self::Legal => "LEGAL",
            Self::Marketing => "MARKETING",
            Self::DevOps => "DEVOPS",
        }
    }

    /// Map to existing AgentConfig ID
    #[must_use]
    pub const fn to_agent_id(&self) -> &'static str {
        match self {
            Self::Orchestrator => "orchestrator",
            Self::Pm => "pm",
            Self::Dev => "backend",
            Self::Qa => "qa",
            Self::Researcher => "researcher",
            Self::Po => "po",
            Self::Hr => "hr",
            Self::Ba => "ba",
            Self::Ux => "ux",
            Self::Cs => "cs",
            Self::Legal => "legal",
            Self::Marketing => "marketing",
            Self::DevOps => "devops",
        }
    }

    /// Return routing priority
    #[must_use]
    pub const fn priority(&self) -> u32 {
        match self {
            Self::Orchestrator => 1000,
            Self::Po => 110,
            Self::Pm => 100,
            Self::Dev => 90,
            Self::Qa => 80,
            Self::Researcher => 70,
            Self::Ba => 65,
            Self::DevOps => 60,
            Self::Legal => 55,
            Self::Ux => 50,
            Self::Marketing => 45,
            Self::Hr => 40,
            Self::Cs => 35,
        }
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for Domain {
    fn default() -> Self {
        Self::Dev
    }
}

#[cfg(test)]
mod tests;

