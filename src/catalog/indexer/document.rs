use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSemanticDocument {
    pub name: String,
    pub title: String,
    pub description: String,
    pub intent: String,
    pub triggers: Vec<String>,
    pub keywords: Vec<String>,
    pub action_triggers: Vec<String>,
    pub file_patterns: Vec<String>,
    pub applicable_phases: Vec<String>,
    pub micro_rules: Vec<String>,
    pub headings: Vec<String>,
    pub summary_snippet: String,
}

impl SkillSemanticDocument {
    /// Extracts structured semantic fields from a skill's Markdown / YAML content.
    pub fn extract(name: &str, content: &str) -> Self {
        super::extractor::extract_semantic_document(name, content)
    }

    /// Formats the extracted semantic document into a high-density, search-optimized passage string.
    pub fn to_passage(&self, max_chars: usize) -> String {
        super::passage::document_to_passage(self, max_chars)
    }

    /// Infers action triggers, file patterns, and applicable phases from name, keywords, and content.
    pub fn infer_metadata(&mut self) {
        super::inference::infer_document_metadata(self);
    }
}
