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
mod tests {
    use super::*;

    #[test]
    fn test_domain_as_str() {
        assert_eq!(Domain::Orchestrator.as_str(), "ORCHESTRATOR");
        assert_eq!(Domain::Pm.as_str(), "PM");
        assert_eq!(Domain::Dev.as_str(), "DEV");
        assert_eq!(Domain::Qa.as_str(), "QA");
        assert_eq!(Domain::Researcher.as_str(), "RESEARCHER");
    }

    #[test]
    fn test_domain_to_agent_id() {
        assert_eq!(Domain::Orchestrator.to_agent_id(), "orchestrator");
        assert_eq!(Domain::Pm.to_agent_id(), "pm");
        assert_eq!(Domain::Dev.to_agent_id(), "backend");
        assert_eq!(Domain::Qa.to_agent_id(), "qa");
        assert_eq!(Domain::Researcher.to_agent_id(), "researcher");
    }

    #[test]
    fn test_domain_priority() {
        assert!(Domain::Orchestrator.priority() > Domain::Pm.priority());
        assert!(Domain::Pm.priority() > Domain::Dev.priority());
        assert!(Domain::Dev.priority() > Domain::Qa.priority());
    }

    #[test]
    fn test_domain_serialize() {
        let domain = Domain::Dev;
        let json = serde_json::to_string(&domain).unwrap();
        assert_eq!(json, r#""DEV""#);
    }

    #[test]
    fn test_domain_deserialize() {
        let domain: Domain = serde_json::from_str(r#""PM""#).unwrap();
        assert_eq!(domain, Domain::Pm);
    }

    #[test]
    fn test_domain_display() {
        assert_eq!(format!("{}", Domain::Qa), "QA");
    }

    #[test]
    fn test_extended_domain_as_str() {
        assert_eq!(Domain::Po.as_str(), "PO");
        assert_eq!(Domain::Hr.as_str(), "HR");
        assert_eq!(Domain::Ba.as_str(), "BA");
        assert_eq!(Domain::Ux.as_str(), "UX");
        assert_eq!(Domain::Cs.as_str(), "CS");
        assert_eq!(Domain::Legal.as_str(), "LEGAL");
        assert_eq!(Domain::Marketing.as_str(), "MARKETING");
        assert_eq!(Domain::DevOps.as_str(), "DEVOPS");
    }

    #[test]
    fn test_extended_domain_to_agent_id() {
        assert_eq!(Domain::Po.to_agent_id(), "po");
        assert_eq!(Domain::Hr.to_agent_id(), "hr");
        assert_eq!(Domain::Ba.to_agent_id(), "ba");
        assert_eq!(Domain::Ux.to_agent_id(), "ux");
        assert_eq!(Domain::Cs.to_agent_id(), "cs");
        assert_eq!(Domain::Legal.to_agent_id(), "legal");
        assert_eq!(Domain::Marketing.to_agent_id(), "marketing");
        assert_eq!(Domain::DevOps.to_agent_id(), "devops");
    }

    #[test]
    fn test_extended_domain_priority() {
        assert!(Domain::Po.priority() > Domain::Pm.priority());
        assert!(Domain::Ba.priority() > Domain::DevOps.priority());
        assert!(Domain::Cs.priority() < Domain::Hr.priority());
    }

    #[test]
    fn test_extended_domain_serialize() {
        let domain = Domain::DevOps;
        let json = serde_json::to_string(&domain).unwrap();
        assert_eq!(json, r#""DEVOPS""#);
    }

    #[test]
    fn test_extended_domain_deserialize() {
        let domain: Domain = serde_json::from_str(r#""PO""#).unwrap();
        assert_eq!(domain, Domain::Po);

        let domain: Domain = serde_json::from_str(r#""DEVOPS""#).unwrap();
        assert_eq!(domain, Domain::DevOps);
    }
}
