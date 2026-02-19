use super::*;

#[test]
fn test_export_format_extension() {
    assert_eq!(ExportFormat::Json.extension(), "json");
    assert_eq!(ExportFormat::Yaml.extension(), "yaml");
}

#[test]
fn test_export_format_from_extension() {
    assert_eq!(
        ExportFormat::from_extension("json"),
        Some(ExportFormat::Json)
    );
    assert_eq!(
        ExportFormat::from_extension("yaml"),
        Some(ExportFormat::Yaml)
    );
    assert_eq!(
        ExportFormat::from_extension("yml"),
        Some(ExportFormat::Yaml)
    );
    assert_eq!(ExportFormat::from_extension("txt"), None);
}

#[test]
fn test_checksum_calculation() {
    let def = PortableSkillDef {
        name: "test".to_string(),
        description: "test skill".to_string(),
        category: "workflow".to_string(),
        trigger: PortableTrigger {
            keywords: vec!["test".to_string()],
            regex_patterns: vec![],
            intents: vec![],
            priority: 0,
        },
        steps: vec![],
        input_schema: None,
        tags: vec![],
    };

    let checksum1 = SkillEcosystem::calculate_checksum(&def);
    let checksum2 = SkillEcosystem::calculate_checksum(&def);

    assert_eq!(checksum1, checksum2);
    assert_eq!(checksum1.len(), 16);
}

#[test]
fn test_portable_skill_serialization() {
    let portable = PortableSkill {
        format_version: "1.0".to_string(),
        skill: Arc::new(PortableSkillDef {
            name: "file_commit".to_string(),
            description: "Read file and commit".to_string(),
            category: "workflow".to_string(),
            trigger: PortableTrigger {
                keywords: vec!["파일 커밋".to_string(), "file commit".to_string()],
                regex_patterns: vec![],
                intents: vec![],
                priority: 10,
            },
            steps: vec![
                PortableStep {
                    order: 1,
                    tool_name: "file_read".to_string(),
                    input_template: serde_json::json!({"path": "{{file_path}}"}),
                    on_error: "abort".to_string(),
                    description: Some("Read the file".to_string()),
                },
                PortableStep {
                    order: 2,
                    tool_name: "git_commit".to_string(),
                    input_template: serde_json::json!({"message": "{{commit_message}}"}),
                    on_error: "abort".to_string(),
                    description: Some("Commit changes".to_string()),
                },
            ],
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"},
                    "commit_message": {"type": "string"}
                },
                "required": ["file_path", "commit_message"]
            })),
            tags: vec!["git".to_string(), "file".to_string()],
        }),
        export_info: Arc::new(ExportInfo {
            exported_at: Utc::now(),
            cratos_version: "0.1.0".to_string(),
            author: Some("cratos-user".to_string()),
            source_url: None,
            license: Some("MIT".to_string()),
        }),
        checksum: "abc123".to_string(),
    };

    // Test YAML serialization
    let yaml = serde_yaml::to_string(&portable).unwrap();
    assert!(yaml.contains("file_commit"));
    assert!(yaml.contains("파일 커밋"));

    // Test JSON serialization
    let json = serde_json::to_string_pretty(&portable).unwrap();
    assert!(json.contains("file_commit"));
}
