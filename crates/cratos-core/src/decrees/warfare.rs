//! Development Rules (Warfare) structure

use serde::{Deserialize, Serialize};

/// Development rules complete structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warfare {
    /// Metadata
    pub meta: WarfareMeta,
    /// Section list
    pub sections: Vec<WarfareSection>,
}

/// Development rules metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarfareMeta {
    /// Title
    pub title: String,
    /// Motto
    pub motto: String,
    /// Related law articles
    #[serde(default)]
    pub law_references: Vec<u8>,
}

/// Development rules section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarfareSection {
    /// Section ID
    pub id: u8,
    /// Section title
    pub title: String,
    /// Sub-rules
    pub rules: Vec<WarfareRule>,
}

/// Individual rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarfareRule {
    /// Rule title
    pub title: String,
    /// Rule items
    pub items: Vec<String>,
    /// Code example (optional)
    #[serde(default)]
    pub code_example: Option<String>,
}

impl Warfare {
    /// Return section count
    #[must_use]
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Find section by ID
    #[must_use]
    pub fn get_section(&self, id: u8) -> Option<&WarfareSection> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// Validate
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.sections.is_empty()
    }

    /// Generate formatted output
    #[must_use]
    pub fn format_display(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("# {}\n\n", self.meta.title));
        output.push_str(&format!("> {}\n\n", self.meta.motto));

        if !self.meta.law_references.is_empty() {
            let refs: Vec<String> = self
                .meta
                .law_references
                .iter()
                .map(|r| format!("Article {}", r))
                .collect();
            output.push_str(&format!("Related Laws: {}\n\n", refs.join(", ")));
        }

        output.push_str("---\n\n");

        for section in &self.sections {
            output.push_str(&format!("## {}. {}\n\n", section.id, section.title));

            for rule in &section.rules {
                output.push_str(&format!("### {}\n\n", rule.title));

                for item in &rule.items {
                    output.push_str(&format!("- {}\n", item));
                }

                if let Some(code) = &rule.code_example {
                    output.push_str(&format!("\n```\n{}\n```\n", code));
                }

                output.push('\n');
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_warfare() -> Warfare {
        Warfare {
            meta: WarfareMeta {
                title: "Development Rules".to_string(),
                motto: "Code is the battlefield".to_string(),
                law_references: vec![2, 10],
            },
            sections: vec![
                WarfareSection {
                    id: 1,
                    title: "Code Quality".to_string(),
                    rules: vec![WarfareRule {
                        title: "Testing".to_string(),
                        items: vec!["Coverage 70% or above".to_string()],
                        code_example: Some("cargo test".to_string()),
                    }],
                },
                WarfareSection {
                    id: 2,
                    title: "Commit Rules".to_string(),
                    rules: vec![WarfareRule {
                        title: "Conventional Commits".to_string(),
                        items: vec!["feat, fix, docs, etc.".to_string()],
                        code_example: None,
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_section_count() {
        let warfare = create_test_warfare();
        assert_eq!(warfare.section_count(), 2);
    }

    #[test]
    fn test_get_section() {
        let warfare = create_test_warfare();

        let section = warfare.get_section(1);
        assert!(section.is_some());
        assert_eq!(section.unwrap().title, "Code Quality");

        assert!(warfare.get_section(99).is_none());
    }

    #[test]
    fn test_is_valid() {
        let warfare = create_test_warfare();
        assert!(warfare.is_valid());

        let empty = Warfare {
            meta: WarfareMeta {
                title: "Empty".to_string(),
                motto: "".to_string(),
                law_references: vec![],
            },
            sections: vec![],
        };
        assert!(!empty.is_valid());
    }

    #[test]
    fn test_format_display() {
        let warfare = create_test_warfare();
        let output = warfare.format_display();

        assert!(output.contains("# Development Rules"));
        assert!(output.contains("Code is the battlefield"));
        assert!(output.contains("Article 2, Article 10"));
        assert!(output.contains("## 1. Code Quality"));
        assert!(output.contains("```\ncargo test\n```"));
    }
}
