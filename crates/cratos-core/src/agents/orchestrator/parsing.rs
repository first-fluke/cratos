use super::super::config::AgentConfig;
use super::types::{OrchestratorError, OrchestratorResult, ParsedAgentTask, MENTION_REGEX};
use std::collections::HashMap;

pub struct AgentTaskParser<'a> {
    pub(crate) agents: &'a HashMap<String, AgentConfig>,
    pub(crate) default_agent: String,
}

impl<'a> AgentTaskParser<'a> {
    pub fn new(agents: &'a HashMap<String, AgentConfig>, default_agent: String) -> Self {
        Self {
            agents,
            default_agent,
        }
    }

    /// Parse input into agent tasks
    ///
    /// Supports:
    /// - Explicit @mentions: "@backend implement API"
    /// - Multiple mentions: "@backend API @frontend UI"
    /// - Semantic routing (if no mention): "implement API" → backend agent
    pub fn parse_input(&self, input: &str) -> OrchestratorResult<Vec<ParsedAgentTask>> {
        let mut tasks = Vec::new();

        // Find all @mentions
        let mentions: Vec<_> = MENTION_REGEX.find_iter(input).collect();

        if mentions.is_empty() {
            // No explicit mention — use default agent
            tasks.push(ParsedAgentTask {
                agent_id: self.default_agent.clone(),
                prompt: input.to_string(),
                explicit_mention: false,
            });
        } else if mentions.len() == 1 {
            // Single mention
            let mention = &mentions[0];
            let agent_id = mention.as_str().trim_start_matches('@').trim().to_string();
            let prompt = input[mention.end()..].trim().to_string();

            // Verify agent exists
            if !self.agents.contains_key(&agent_id) {
                return Err(OrchestratorError::AgentNotFound(agent_id));
            }

            tasks.push(ParsedAgentTask {
                agent_id,
                prompt,
                explicit_mention: true,
            });
        } else {
            // Multiple mentions - split into tasks
            for (i, mention) in mentions.iter().enumerate() {
                let agent_id = mention.as_str().trim_start_matches('@').trim().to_string();

                // Verify agent exists
                if !self.agents.contains_key(&agent_id) {
                    return Err(OrchestratorError::AgentNotFound(agent_id));
                }

                // Get prompt until next mention or end
                let start = mention.end();
                let end = if i + 1 < mentions.len() {
                    mentions[i + 1].start()
                } else {
                    input.len()
                };
                let prompt = input[start..end].trim().to_string();

                if !prompt.is_empty() {
                    tasks.push(ParsedAgentTask {
                        agent_id,
                        prompt,
                        explicit_mention: true,
                    });
                }
            }
        }

        Ok(tasks)
    }
}
