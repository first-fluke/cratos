//! Laws structure

use serde::{Deserialize, Serialize};

/// Laws complete structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Laws {
    /// Metadata
    pub meta: LawsMeta,
    /// Article list
    pub articles: Vec<Article>,
}

/// Laws metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LawsMeta {
    /// Title
    pub title: String,
    /// Philosophy/motto
    pub philosophy: String,
    /// Immutable flag
    #[serde(default = "default_immutable")]
    pub immutable: bool,
}

fn default_immutable() -> bool {
    true
}

/// Individual article
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    /// Article number (1-10)
    pub id: u8,
    /// Article title
    pub title: String,
    /// Rules list
    pub rules: Vec<String>,
}

impl Laws {
    /// Return article count
    #[must_use]
    pub fn article_count(&self) -> usize {
        self.articles.len()
    }

    /// Find article by ID
    #[must_use]
    pub fn get_article(&self, id: u8) -> Option<&Article> {
        self.articles.iter().find(|a| a.id == id)
    }

    /// Validate (requires 10 articles)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.articles.len() >= 10
    }

    /// Generate formatted output
    #[must_use]
    pub fn format_display(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("# {}\n\n", self.meta.title));
        output.push_str(&format!("> \"{}\"\n\n", self.meta.philosophy));
        output.push_str("---\n\n");

        for article in &self.articles {
            output.push_str(&format!(
                "## Article {} ({})\n\n",
                article.id, article.title
            ));
            for rule in &article.rules {
                output.push_str(&format!("- {}\n", rule));
            }
            output.push('\n');
        }

        output
    }
}

impl Article {
    /// Generate article reference string (e.g., "Article 2")
    #[must_use]
    pub fn reference(&self) -> String {
        format!("Article {}", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_laws() -> Laws {
        Laws {
            meta: LawsMeta {
                title: "Laws (LAWS)".to_string(),
                philosophy: "Test philosophy".to_string(),
                immutable: true,
            },
            articles: vec![
                Article {
                    id: 1,
                    title: "Planning and Design".to_string(),
                    rules: vec!["Rule 1".to_string(), "Rule 2".to_string()],
                },
                Article {
                    id: 2,
                    title: "Development Guidelines".to_string(),
                    rules: vec!["Rule A".to_string()],
                },
            ],
        }
    }

    #[test]
    fn test_article_count() {
        let laws = create_test_laws();
        assert_eq!(laws.article_count(), 2);
    }

    #[test]
    fn test_get_article() {
        let laws = create_test_laws();

        let article = laws.get_article(1);
        assert!(article.is_some());
        assert_eq!(article.unwrap().title, "Planning and Design");

        assert!(laws.get_article(99).is_none());
    }

    #[test]
    fn test_is_valid() {
        let laws = create_test_laws();
        assert!(!laws.is_valid()); // Only 2 articles
    }

    #[test]
    fn test_article_reference() {
        let article = Article {
            id: 2,
            title: "Development Guidelines".to_string(),
            rules: vec![],
        };
        assert_eq!(article.reference(), "Article 2");
    }

    #[test]
    fn test_format_display() {
        let laws = create_test_laws();
        let output = laws.format_display();

        assert!(output.contains("# Laws (LAWS)"));
        assert!(output.contains("Test philosophy"));
        assert!(output.contains("## Article 1 (Planning and Design)"));
        assert!(output.contains("- Rule 1"));
    }
}
