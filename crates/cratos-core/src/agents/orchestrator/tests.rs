use super::*;
use super::parsing::AgentTaskParser;

#[test]
fn test_parse_single_mention() {
    let orchestrator = AgentOrchestrator::default();
    let parser = AgentTaskParser::new(&orchestrator.agents, orchestrator.config.default_agent.clone());
    let tasks = parser.parse_input("@backend implement the API").unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].agent_id, "backend");
    assert_eq!(tasks[0].prompt, "implement the API");
    assert!(tasks[0].explicit_mention);
}
