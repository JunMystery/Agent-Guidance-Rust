use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSemanticDocument {
    pub name: String,
    pub title: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub keywords: Vec<String>,
    pub headings: Vec<String>,
    pub summary_snippet: String,
}

impl SkillSemanticDocument {
    /// Extracts structured semantic fields from a skill's Markdown / YAML content.
    pub fn extract(name: &str, content: &str) -> Self {
        let mut doc = Self {
            name: name.to_string(),
            ..Default::default()
        };

        let mut lines = content.lines().peekable();
        let mut in_frontmatter = false;
        let mut frontmatter_lines = Vec::new();
        let mut body_lines = Vec::new();

        // 1. Separate YAML frontmatter if present
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

        // 2. Parse Frontmatter
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
            } else if let Some(rest) = trimmed.strip_prefix("tools:") {
                current_key = "tools".to_string();
                for t in rest.split(',') {
                    let cleaned = t.trim().trim_matches('"').trim_matches('\'');
                    if !cleaned.is_empty() {
                        doc.keywords.push(cleaned.to_string());
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("tags:") {
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
                let bullet = trimmed.trim_start_matches("- ").trim();
                desc_accumulator.push(bullet.to_string());
            } else if current_key == "description" && !trimmed.contains(':') {
                desc_accumulator.push(trimmed.to_string());
            }
        }

        if !desc_accumulator.is_empty() {
            doc.description = desc_accumulator.join(" ");
        }

        // 3. Parse Markdown Body for Headings, Title, and Triggers
        let mut in_trigger_section = false;
        let mut trigger_count = 0;
        let mut first_heading_captured = false;
        let mut body_text_collector = Vec::new();

        for line in &body_lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Capture H1 title if not set
            if trimmed.starts_with("# ") && !first_heading_captured {
                doc.title = trimmed.trim_start_matches("# ").trim().to_string();
                first_heading_captured = true;
                continue;
            }

            // Check Section Headings
            if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
                let heading_title = trimmed.trim_start_matches('#').trim().to_string();
                let lower = heading_title.to_lowercase();

                doc.headings.push(heading_title);

                in_trigger_section = lower.contains("when to")
                    || lower.contains("activate")
                    || lower.contains("use when")
                    || lower.contains("trigger")
                    || lower.contains("intent")
                    || lower.contains("capabilities")
                    || lower.contains("overview");
                continue;
            }

            // Extract Triggers / Bullet Points
            if in_trigger_section && (trimmed.starts_with("- ") || trimmed.starts_with("* ")) {
                let item = trimmed.trim_start_matches("- ").trim_start_matches("* ").trim();
                if !item.is_empty() && trigger_count < 6 {
                    doc.triggers.push(item.to_string());
                    trigger_count += 1;
                }
            } else if body_text_collector.len() < 6 && !trimmed.starts_with("```") && !trimmed.starts_with('#') {
                // Collect early body text as summary if description was empty
                body_text_collector.push(trimmed);
            }
        }

        if doc.description.is_empty() && !body_text_collector.is_empty() {
            doc.description = body_text_collector.join(" ");
        }

        // 4. Extract automated domain keywords from title and name
        let mut kw_set = HashSet::new();
        for kw in &doc.keywords {
            kw_set.insert(kw.to_lowercase());
        }

        let name_clean = doc.name.replace('-', " ").replace('_', " ");
        for word in name_clean.split_whitespace() {
            if word.len() >= 3 && !matches!(word, "and" | "the" | "for" | "with" | "cheat" | "sheet") {
                kw_set.insert(word.to_lowercase());
            }
        }

        doc.keywords = kw_set.into_iter().collect();
        doc.keywords.sort();

        // 5. Build summary snippet
        doc.summary_snippet = doc.to_passage(1500);
        doc
    }

    /// Formats the extracted semantic document into a high-density, search-optimized passage string.
    pub fn to_passage(&self, max_chars: usize) -> String {
        let mut parts = Vec::new();

        // 1. Skill Name & Title
        if !self.title.is_empty() && self.title.to_lowercase() != self.name.to_lowercase() {
            parts.push(format!("Skill: {} ({})", self.name, self.title));
        } else {
            parts.push(format!("Skill: {}", self.name));
        }

        // 2. Full Description
        if !self.description.is_empty() {
            parts.push(format!("Description: {}", self.description));
        }

        // 3. Triggers & Activation Scenarios
        if !self.triggers.is_empty() {
            parts.push(format!("Triggers: {}", self.triggers.join("; ")));
        }

        // 4. Keywords & Domain Tags
        if !self.keywords.is_empty() {
            parts.push(format!("Keywords: {}", self.keywords.join(", ")));
        }

        // 5. Headings / Concepts
        if !self.headings.is_empty() {
            let sample_headings: Vec<String> = self.headings.iter().take(4).cloned().collect();
            parts.push(format!("Topics: {}", sample_headings.join(", ")));
        }

        let full = parts.join("\n");
        if full.len() > max_chars {
            full.chars().take(max_chars).collect()
        } else {
            full
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_skill_semantic_document() {
        let sample = r#"---
name: agent-eval
description: Head-to-head comparison of coding agents with pass rate, cost, time, and consistency metrics. Use when choosing between coding agents.
license: MIT
tools: Read, Write, Edit, Bash
---

# Agent Eval Skill

A lightweight CLI tool for comparing coding agents.

## When to Activate

- Comparing coding agents (Claude Code, Aider, Codex) on custom tasks
- Measuring agent performance before adopting a new model
- Running regression checks on agent setups
- Producing data-backed agent selection decisions

## Core Concepts

### YAML Task Definitions
Define tasks declaratively with judge criteria.
"#;

        let doc = SkillSemanticDocument::extract("agent-eval", sample);
        assert_eq!(doc.name, "agent-eval");
        assert!(doc.description.contains("Head-to-head comparison"));
        assert_eq!(doc.triggers.len(), 4);
        assert!(doc.triggers[0].contains("Comparing coding agents"));
        assert!(doc.keywords.contains(&"bash".to_string()));

        let passage = doc.to_passage(1500);
        assert!(passage.contains("Skill: agent-eval"));
        assert!(passage.contains("Description:"));
        assert!(passage.contains("Triggers:"));
        assert!(passage.contains("Comparing coding agents"));
    }

    #[test]
    fn test_extract_plain_cheatsheet() {
        let sample = r#"# SQL Injection Prevention Cheat Sheet

Defense in depth against SQL injection attacks in web applications.

## Primary Defenses
- Use Parameterized Queries
- Use Stored Procedures
- Allow-list Input Validation
"#;

        let doc = SkillSemanticDocument::extract("sql-injection-prevention-cheat-sheet", sample);
        assert_eq!(doc.name, "sql-injection-prevention-cheat-sheet");
        assert_eq!(doc.title, "SQL Injection Prevention Cheat Sheet");
        assert!(doc.description.contains("Defense in depth against SQL injection"));
        let passage = doc.to_passage(1500);
        assert!(passage.contains("SQL Injection Prevention Cheat Sheet"));
    }
}
