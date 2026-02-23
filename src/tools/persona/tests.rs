use super::*;

#[test]
fn test_persona_tool_name() {
    let tool = PersonaTool::new();
    assert_eq!(tool.definition().name, "persona_info");
}
