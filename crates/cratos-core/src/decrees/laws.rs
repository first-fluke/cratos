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
mod tests;

