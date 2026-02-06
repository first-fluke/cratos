//! Agent Configuration
//!
//! Configuration types for defining agents and their behavior.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Unique agent identifier (e.g., "backend", "frontend")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Agent description
    pub description: String,
    /// Agent persona (system prompt and behavior)
    pub persona: AgentPersona,
    /// CLI provider configuration
    pub cli: CliProviderConfig,
    /// Tool permissions
    pub tools: AgentToolConfig,
    /// Routing configuration
    pub routing: AgentRouting,
    /// Whether the agent is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Agent persona configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPersona {
    /// System prompt / character description
    pub prompt: String,
    /// Specialized capabilities
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Preferred response style
    #[serde(default = "default_response_style")]
    pub response_style: String,
}

fn default_response_style() -> String {
    "concise".to_string()
}

impl Default for AgentPersona {
    fn default() -> Self {
        Self {
            prompt: "You are a helpful assistant.".to_string(),
            capabilities: Vec::new(),
            response_style: default_response_style(),
        }
    }
}

/// CLI provider configuration for the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliProviderConfig {
    /// Provider name (e.g., "claude", "gemini", "groq")
    pub provider: String,
    /// Model to use
    #[serde(default)]
    pub model: Option<String>,
    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    300 // 5 minutes
}

impl Default for CliProviderConfig {
    fn default() -> Self {
        Self {
            provider: "groq".to_string(), // Default to free tier
            model: None,
            timeout_seconds: default_timeout(),
        }
    }
}

/// Agent tool permissions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentToolConfig {
    /// Allowed tools (empty = all allowed)
    #[serde(default)]
    pub allow: HashSet<String>,
    /// Denied tools
    #[serde(default)]
    pub deny: HashSet<String>,
}

impl AgentToolConfig {
    /// Check if a tool is allowed
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        // Deny list takes precedence
        if self.deny.contains(tool_name) {
            return false;
        }
        // If allow list is empty, all tools are allowed
        if self.allow.is_empty() {
            return true;
        }
        // Check allow list
        self.allow.contains(tool_name)
    }

    /// Create a permissive config (all tools allowed)
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Create a restrictive config with specific allowed tools
    pub fn with_allowed(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allow: tools.into_iter().map(Into::into).collect(),
            deny: HashSet::new(),
        }
    }
}

/// Agent routing configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRouting {
    /// Keywords that trigger this agent
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Intent patterns (regex)
    #[serde(default)]
    pub intents: Vec<String>,
    /// Priority (higher = checked first)
    #[serde(default)]
    pub priority: u32,
}

impl AgentConfig {
    /// Create a new agent config with basic settings
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            persona: AgentPersona::default(),
            cli: CliProviderConfig::default(),
            tools: AgentToolConfig::default(),
            routing: AgentRouting::default(),
            enabled: true,
        }
    }

    /// Create a backend developer agent
    pub fn backend() -> Self {
        Self {
            id: "backend".to_string(),
            name: "Backend Developer".to_string(),
            description: "API, database, server logic specialist".to_string(),
            persona: AgentPersona {
                prompt: r#"You are a backend specialist. You excel at:
- API design (REST, GraphQL)
- Database modeling and queries
- Server-side logic and architecture
- Performance optimization
- Security best practices"#
                    .to_string(),
                capabilities: vec![
                    "api_design".to_string(),
                    "database".to_string(),
                    "security".to_string(),
                ],
                response_style: "technical".to_string(),
            },
            cli: CliProviderConfig {
                provider: "anthropic".to_string(), // Complex reasoning
                model: Some("claude-sonnet-4-20250514".to_string()),
                timeout_seconds: 300,
            },
            tools: AgentToolConfig::default(),
            routing: AgentRouting {
                keywords: vec![
                    "API".to_string(),
                    "database".to_string(),
                    "server".to_string(),
                    "backend".to_string(),
                    "DB".to_string(),
                    "SQL".to_string(),
                    "endpoint".to_string(),
                ],
                intents: vec!["api_design".to_string(), "database_query".to_string()],
                priority: 100,
            },
            enabled: true,
        }
    }

    /// Create a frontend developer agent
    pub fn frontend() -> Self {
        Self {
            id: "frontend".to_string(),
            name: "Frontend Developer".to_string(),
            description: "UI/UX, web components specialist".to_string(),
            persona: AgentPersona {
                prompt: r#"You are a frontend specialist. You excel at:
- React/Vue/Svelte component design
- CSS/Tailwind styling
- User experience optimization
- Accessibility (a11y)
- State management"#
                    .to_string(),
                capabilities: vec![
                    "react".to_string(),
                    "css".to_string(),
                    "accessibility".to_string(),
                ],
                response_style: "visual".to_string(),
            },
            cli: CliProviderConfig {
                provider: "gemini".to_string(), // Fast UI generation
                model: Some("gemini-2.0-flash".to_string()),
                timeout_seconds: 180,
            },
            tools: AgentToolConfig::default(),
            routing: AgentRouting {
                keywords: vec![
                    "UI".to_string(),
                    "frontend".to_string(),
                    "component".to_string(),
                    "CSS".to_string(),
                    "React".to_string(),
                    "page".to_string(),
                    "screen".to_string(),
                ],
                intents: vec!["ui_design".to_string(), "component_creation".to_string()],
                priority: 100,
            },
            enabled: true,
        }
    }

    /// Create a QA engineer agent
    pub fn qa() -> Self {
        Self {
            id: "qa".to_string(),
            name: "QA Engineer".to_string(),
            description: "Testing, security, quality assurance specialist".to_string(),
            persona: AgentPersona {
                prompt: r#"You are a QA specialist. You excel at:
- Writing unit, integration, and e2e tests
- Security vulnerability assessment
- Code review for bugs and issues
- Test coverage analysis
- Performance testing"#
                    .to_string(),
                capabilities: vec![
                    "testing".to_string(),
                    "security".to_string(),
                    "code_review".to_string(),
                ],
                response_style: "detailed".to_string(),
            },
            cli: CliProviderConfig {
                provider: "gemini".to_string(), // Fast test generation
                model: Some("gemini-2.0-flash".to_string()),
                timeout_seconds: 180,
            },
            tools: AgentToolConfig::default(),
            routing: AgentRouting {
                keywords: vec![
                    "test".to_string(),
                    "testing".to_string(),
                    "QA".to_string(),
                    "security".to_string(),
                    "verification".to_string(),
                    "bug".to_string(),
                ],
                intents: vec!["test_creation".to_string(), "security_review".to_string()],
                priority: 90,
            },
            enabled: true,
        }
    }

    /// Create a PM/Planner agent
    pub fn pm() -> Self {
        Self {
            id: "pm".to_string(),
            name: "Product Manager".to_string(),
            description: "Planning, analysis, roadmap specialist".to_string(),
            persona: AgentPersona {
                prompt: r#"You are a product manager specialist. You excel at:
- Requirements analysis
- Project planning and roadmaps
- Technical specifications
- User story creation
- Prioritization and scoping"#
                    .to_string(),
                capabilities: vec![
                    "planning".to_string(),
                    "analysis".to_string(),
                    "documentation".to_string(),
                ],
                response_style: "structured".to_string(),
            },
            cli: CliProviderConfig {
                provider: "anthropic".to_string(), // Complex analysis
                model: Some("claude-sonnet-4-20250514".to_string()),
                timeout_seconds: 300,
            },
            tools: AgentToolConfig::with_allowed(["search", "read_file", "list_files"]),
            routing: AgentRouting {
                keywords: vec![
                    "plan".to_string(),
                    "planning".to_string(),
                    "analysis".to_string(),
                    "PM".to_string(),
                    "roadmap".to_string(),
                    "requirements".to_string(),
                ],
                intents: vec!["planning".to_string(), "analysis".to_string()],
                priority: 80,
            },
            enabled: true,
        }
    }

    /// Create a researcher agent
    pub fn researcher() -> Self {
        Self {
            id: "researcher".to_string(),
            name: "Researcher".to_string(),
            description: "Research, documentation, information gathering specialist".to_string(),
            persona: AgentPersona {
                prompt: r#"You are a research specialist. You excel at:
- Information gathering and synthesis
- Documentation and summarization
- Comparative analysis
- Technical research
- Best practices research"#
                    .to_string(),
                capabilities: vec![
                    "research".to_string(),
                    "documentation".to_string(),
                    "analysis".to_string(),
                ],
                response_style: "thorough".to_string(),
            },
            cli: CliProviderConfig {
                provider: "anthropic".to_string(),
                model: Some("claude-sonnet-4-20250514".to_string()),
                timeout_seconds: 300,
            },
            tools: AgentToolConfig::with_allowed(["search", "read_file", "http_get"]),
            routing: AgentRouting {
                keywords: vec![
                    "research".to_string(),
                    "investigate".to_string(),
                    "find".to_string(),
                    "compare".to_string(),
                    "analyze".to_string(),
                ],
                intents: vec!["research".to_string(), "comparison".to_string()],
                priority: 70,
            },
            enabled: true,
        }
    }

    /// Create a Product Owner agent
    pub fn po() -> Self {
        Self {
            id: "po".to_string(),
            name: "Product Owner".to_string(),
            description: "Product vision, roadmap, and prioritization specialist".to_string(),
            persona: AgentPersona {
                prompt: "You are a product owner specialist. You excel at product vision, roadmap planning, stakeholder alignment, and prioritization."
                    .to_string(),
                capabilities: vec!["roadmap".to_string(), "prioritization".to_string(), "okr".to_string()],
                response_style: "strategic".to_string(),
            },
            cli: CliProviderConfig {
                provider: "anthropic".to_string(),
                model: Some("claude-sonnet-4-20250514".to_string()),
                timeout_seconds: 300,
            },
            tools: AgentToolConfig::with_allowed(["search", "read_file", "list_files"]),
            routing: AgentRouting {
                keywords: vec!["product".to_string(), "roadmap".to_string(), "prioritize".to_string(), "OKR".to_string()],
                intents: vec!["product_planning".to_string()],
                priority: 110,
            },
            enabled: true,
        }
    }

    /// Create an HR agent
    pub fn hr() -> Self {
        Self::new("hr", "HR Specialist", "Team management and culture specialist")
    }

    /// Create a Business Analyst agent
    pub fn ba() -> Self {
        Self::new("ba", "Business Analyst", "Requirements analysis and process mapping specialist")
    }

    /// Create a UX Designer agent
    pub fn ux() -> Self {
        Self {
            id: "ux".to_string(),
            name: "UX Designer".to_string(),
            description: "User experience and interface design specialist".to_string(),
            persona: AgentPersona {
                prompt: "You are a UX design specialist. You excel at user research, prototyping, design systems, and accessibility."
                    .to_string(),
                capabilities: vec!["ui_design".to_string(), "prototyping".to_string(), "accessibility".to_string()],
                response_style: "visual".to_string(),
            },
            cli: CliProviderConfig {
                provider: "gemini".to_string(),
                model: Some("gemini-2.0-flash".to_string()),
                timeout_seconds: 180,
            },
            tools: AgentToolConfig::default(),
            routing: AgentRouting {
                keywords: vec!["UX".to_string(), "design".to_string(), "prototype".to_string(), "accessibility".to_string()],
                intents: vec!["ux_design".to_string()],
                priority: 50,
            },
            enabled: true,
        }
    }

    /// Create a Customer Support agent
    pub fn cs() -> Self {
        Self::new("cs", "Customer Support", "User advocacy and issue resolution specialist")
    }

    /// Create a Legal agent
    pub fn legal() -> Self {
        Self::new("legal", "Legal Advisor", "Compliance, licensing, and privacy specialist")
    }

    /// Create a Marketing agent
    pub fn marketing() -> Self {
        Self::new("marketing", "Marketing Specialist", "Growth, content strategy, and brand management specialist")
    }

    /// Create a DevOps agent
    pub fn devops() -> Self {
        Self {
            id: "devops".to_string(),
            name: "DevOps Engineer".to_string(),
            description: "Infrastructure, CI/CD, and reliability specialist".to_string(),
            persona: AgentPersona {
                prompt: "You are a DevOps specialist. You excel at CI/CD, container orchestration, monitoring, and incident response."
                    .to_string(),
                capabilities: vec!["ci_cd".to_string(), "containers".to_string(), "monitoring".to_string()],
                response_style: "operational".to_string(),
            },
            cli: CliProviderConfig {
                provider: "anthropic".to_string(),
                model: Some("claude-sonnet-4-20250514".to_string()),
                timeout_seconds: 300,
            },
            tools: AgentToolConfig::default(),
            routing: AgentRouting {
                keywords: vec!["deploy".to_string(), "CI".to_string(), "CD".to_string(), "Docker".to_string(), "K8s".to_string(), "infrastructure".to_string()],
                intents: vec!["deployment".to_string(), "infrastructure".to_string()],
                priority: 60,
            },
            enabled: true,
        }
    }

    /// Get all default agents
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::backend(),
            Self::frontend(),
            Self::qa(),
            Self::pm(),
            Self::researcher(),
            Self::po(),
            Self::hr(),
            Self::ba(),
            Self::ux(),
            Self::cs(),
            Self::legal(),
            Self::marketing(),
            Self::devops(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_new() {
        let agent = AgentConfig::new("test", "Test Agent", "A test agent");
        assert_eq!(agent.id, "test");
        assert!(agent.enabled);
    }

    #[test]
    fn test_agent_tool_config_permissive() {
        let config = AgentToolConfig::permissive();
        assert!(config.is_tool_allowed("any_tool"));
        assert!(config.is_tool_allowed("shell"));
    }

    #[test]
    fn test_agent_tool_config_restricted() {
        let config = AgentToolConfig::with_allowed(["read_file", "search"]);
        assert!(config.is_tool_allowed("read_file"));
        assert!(config.is_tool_allowed("search"));
        assert!(!config.is_tool_allowed("shell"));
    }

    #[test]
    fn test_agent_tool_config_deny_priority() {
        let mut config = AgentToolConfig::default();
        config.deny.insert("dangerous".to_string());
        assert!(!config.is_tool_allowed("dangerous"));
    }

    #[test]
    fn test_default_agents() {
        let agents = AgentConfig::defaults();
        assert!(!agents.is_empty());

        let backend = agents.iter().find(|a| a.id == "backend");
        assert!(backend.is_some());

        let frontend = agents.iter().find(|a| a.id == "frontend");
        assert!(frontend.is_some());
    }
}
