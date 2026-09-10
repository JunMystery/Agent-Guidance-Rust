use super::document::SkillSemanticDocument;

/// Extracts structured semantic fields from a skill's Markdown / YAML content.
pub fn extract_semantic_document(name: &str, content: &str) -> SkillSemanticDocument {
    let mut doc = SkillSemanticDocument {
        name: name.to_string(),
        ..Default::default()
    };

    let mut lines = content.lines().peekable();
    let mut in_frontmatter = false;
    let mut frontmatter_lines = Vec::new();
    let mut body_lines = Vec::new();

    if let Some(first_line) = lines.peek() {
        if first_line.trim() == "---" {
            in_frontmatter = true;
            lines.next();
        }
    }

    while let Some(line) = lines.next() {
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
                continue;
            }
            frontmatter_lines.push(line);
        } else {
            body_lines.push(line);
        }
    }

    // Parse Frontmatter
    let mut current_key = String::new();
    let mut desc_accumulator = Vec::new();

    for line in &frontmatter_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("name:") {
            doc.name = rest.trim().trim_matches('"').trim_matches('\'').to_string();
            current_key = "name".to_string();
        } else if let Some(rest) = trimmed.strip_prefix("description:") {
            current_key = "description".to_string();
            let desc_val = rest.trim().trim_matches('"').trim_matches('\'');
            if !desc_val.is_empty() && desc_val != ">" && desc_val != "|" {
                desc_accumulator.push(desc_val.to_string());
            }
        } else if let Some(rest) = trimmed.strip_prefix("tools:").or_else(|| trimmed.strip_prefix("tags:")) {
            current_key = "tags".to_string();
            for t in rest.split(',') {
                let cleaned = t.trim().trim_matches('[').trim_matches(']').trim_matches('"').trim_matches('\'');
                if !cleaned.is_empty() {
                    doc.keywords.push(cleaned.to_string());
                }
            }
        } else if trimmed.starts_with("- ") && current_key == "tags" {
            let tag = trimmed.trim_start_matches("- ").trim().trim_matches('"').trim_matches('\'');
            if !tag.is_empty() {
                doc.keywords.push(tag.to_string());
            }
        } else if trimmed.starts_with("- ") && current_key == "description" {
            desc_accumulator.push(trimmed.trim_start_matches("- ").trim().to_string());
        } else if current_key == "description" && !trimmed.contains(':') {
            desc_accumulator.push(trimmed.to_string());
        }
    }

    if !desc_accumulator.is_empty() {
        doc.description = desc_accumulator.join(" ");
    }

    // Parse Markdown Body
    let mut in_trigger_section = false;
    let mut in_rules_section = false;
    let mut trigger_count = 0;
    let mut first_heading_captured = false;
    let mut body_text_collector = Vec::new();

    for line in &body_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("# ") && !first_heading_captured {
            doc.title = trimmed.trim_start_matches("# ").trim().to_string();
            first_heading_captured = true;
            continue;
        }

        if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
            let heading_title = trimmed.trim_start_matches('#').trim().to_string();
            let lower = heading_title.to_lowercase();
            doc.headings.push(heading_title);

            in_trigger_section = lower.contains("when to")
                || lower.contains("activate")
                || lower.contains("use when")
                || lower.contains("trigger")
                || lower.contains("intent")
                || lower.contains("overview");

            in_rules_section = lower.contains("guideline")
                || lower.contains("rule")
                || lower.contains("standard")
                || lower.contains("practice")
                || lower.contains("core concept")
                || lower.contains("defense")
                || lower.contains("pattern");
            continue;
        }

        if in_trigger_section && (trimmed.starts_with("- ") || trimmed.starts_with("* ")) {
            let item = trimmed.trim_start_matches("- ").trim_start_matches("* ").trim();
            if !item.is_empty() && trigger_count < 8 {
                doc.triggers.push(item.to_string());
                trigger_count += 1;
            }
        } else if in_rules_section && (trimmed.starts_with("- ") || trimmed.starts_with("* ")) {
            let item = trimmed.trim_start_matches("- ").trim_start_matches("* ").trim();
            if !item.is_empty() && doc.micro_rules.len() < 3 && item.len() < 140 {
                doc.micro_rules.push(item.to_string());
            }
        } else if body_text_collector.len() < 6 && !trimmed.starts_with("```") && !trimmed.starts_with('#') {
            body_text_collector.push(trimmed);
        }
    }

    if doc.description.is_empty() && !body_text_collector.is_empty() {
        doc.description = body_text_collector.join(" ");
    }

    // Set concise Intent
    doc.intent = if !doc.description.is_empty() {
        doc.description.split('.').next().unwrap_or(&doc.description).trim().to_string()
    } else if !doc.title.is_empty() {
        doc.title.clone()
    } else {
        doc.name.replace('-', " ")
    };

    // Fallback micro_rules from triggers or description
    if doc.micro_rules.is_empty() {
        for tr in doc.triggers.iter().take(3) {
            if tr.len() < 140 {
                doc.micro_rules.push(tr.clone());
            }
        }
    }
    if doc.micro_rules.is_empty() && !doc.description.is_empty() {
        doc.micro_rules.push(doc.intent.clone());
    }

    // Infer Action Triggers, File Patterns, and Applicable Phases
    doc.infer_metadata();

    // Build summary snippet
    doc.summary_snippet = doc.to_passage(1500);
    doc
}
