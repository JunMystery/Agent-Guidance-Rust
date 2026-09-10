use super::document::SkillSemanticDocument;

/// Formats the extracted semantic document into a high-density, search-optimized passage string.
pub fn document_to_passage(doc: &SkillSemanticDocument, max_chars: usize) -> String {
    let mut parts = Vec::new();

    if !doc.title.is_empty() && doc.title.to_lowercase() != doc.name.to_lowercase() {
        parts.push(format!("Skill: {} ({})", doc.name, doc.title));
    } else {
        parts.push(format!("Skill: {}", doc.name));
    }

    if !doc.intent.is_empty() {
        parts.push(format!("Intent: {}", doc.intent));
    } else if !doc.description.is_empty() {
        parts.push(format!("Description: {}", doc.description));
    }

    if !doc.micro_rules.is_empty() {
        parts.push(format!("Key Rules: {}", doc.micro_rules.join(" | ")));
    }

    if !doc.triggers.is_empty() {
        parts.push(format!("Triggers: {}", doc.triggers.join("; ")));
    }

    if !doc.action_triggers.is_empty() {
        parts.push(format!("Actions: {}", doc.action_triggers.join(", ")));
    }

    let full = parts.join("\n");
    if full.len() > max_chars {
        full.chars().take(max_chars).collect()
    } else {
        full
    }
}
