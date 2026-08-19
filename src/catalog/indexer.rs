use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
        let mut doc = Self {
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

    /// Infers action triggers, file patterns, and applicable phases from name, keywords, and content.
    fn infer_metadata(&mut self) {
        let name_lower = self.name.to_lowercase();
        let desc_lower = self.description.to_lowercase();
        let triggers_lower = self.triggers.join(" ").to_lowercase();
        let combined = format!("{} {} {}", name_lower, desc_lower, triggers_lower);

        let mut actions = HashSet::new();
        let mut patterns = HashSet::new();
        let mut phases = HashSet::new();

        // 1. Action Triggers
        let domain_actions = [
            ("sql", "query"), ("database", "migration"), ("db", "database"),
            ("docker", "container"), ("k8s", "deploy"), ("test", "testing"),
            ("bench", "benchmark"), ("auth", "security"), ("security", "audit"),
            ("clean", "refactor"), ("perf", "optimize"), ("async", "concurrency"),
            ("api", "endpoint"), ("error", "error-handling"), ("log", "telemetry"),
            ("cache", "caching"), ("jwt", "authentication"), ("orm", "data-model"),
        ];
        for (kw, act) in domain_actions {
            if combined.contains(kw) {
                actions.insert(act.to_string());
                actions.insert(kw.to_string());
            }
        }

        // Add words from name
        for word in self.name.replace('-', " ").replace('_', " ").split_whitespace() {
            if word.len() >= 3 && !matches!(word, "and" | "the" | "for" | "with" | "cheat" | "sheet") {
                actions.insert(word.to_lowercase());
            }
        }

        // 2. File Patterns
        if combined.contains("sql") || combined.contains("database") || combined.contains("migration") {
            patterns.insert("*.sql".to_string());
            patterns.insert("*repo*".to_string());
            patterns.insert("*migration*".to_string());
        }
        if combined.contains("docker") || combined.contains("container") {
            patterns.insert("Dockerfile*".to_string());
            patterns.insert("compose*.yml".to_string());
        }
        if combined.contains("test") {
            patterns.insert("*_test.*".to_string());
            patterns.insert("tests/*".to_string());
        }
        if combined.contains("api") || combined.contains("route") || combined.contains("endpoint") {
            patterns.insert("routes/*".to_string());
            patterns.insert("controllers/*".to_string());
        }
        if name_lower.contains("rust") { patterns.insert("*.rs".to_string()); }
        if name_lower.contains("python") || name_lower.contains("django") || name_lower.contains("fastapi") { patterns.insert("*.py".to_string()); }
        if name_lower.contains("react") || name_lower.contains("vue") || name_lower.contains("typescript") { patterns.insert("*.ts".to_string()); patterns.insert("*.tsx".to_string()); }
        if name_lower.contains("go") || name_lower.contains("golang") { patterns.insert("*.go".to_string()); }

        // 3. Applicable Phases
        if combined.contains("test") {
            phases.insert("test".to_string());
            phases.insert("review".to_string());
        }
        if combined.contains("debug") || combined.contains("error") || combined.contains("recovery") {
            phases.insert("debug".to_string());
            phases.insert("implement".to_string());
        }
        if combined.contains("plan") || combined.contains("architecture") || combined.contains("standards") {
            phases.insert("plan".to_string());
            phases.insert("review".to_string());
        }
        if phases.is_empty() {
            phases.insert("implement".to_string());
            phases.insert("refactor".to_string());
        }

        self.action_triggers = actions.into_iter().collect();
        self.action_triggers.sort();
        self.file_patterns = patterns.into_iter().collect();
        self.file_patterns.sort();
        self.applicable_phases = phases.into_iter().collect();
        self.applicable_phases.sort();

        // Merge action triggers into keywords
        let mut kw_set: HashSet<String> = self.keywords.iter().cloned().collect();
        for act in &self.action_triggers {
            kw_set.insert(act.clone());
        }
        self.keywords = kw_set.into_iter().collect();
        self.keywords.sort();
    }

    /// Formats the extracted semantic document into a high-density, search-optimized passage string.
    pub fn to_passage(&self, max_chars: usize) -> String {
        let mut parts = Vec::new();

        if !self.title.is_empty() && self.title.to_lowercase() != self.name.to_lowercase() {
            parts.push(format!("Skill: {} ({})", self.name, self.title));
        } else {
            parts.push(format!("Skill: {}", self.name));
        }

        if !self.intent.is_empty() {
            parts.push(format!("Intent: {}", self.intent));
        } else if !self.description.is_empty() {
            parts.push(format!("Description: {}", self.description));
        }

        if !self.micro_rules.is_empty() {
            parts.push(format!("Key Rules: {}", self.micro_rules.join(" | ")));
        }

        if !self.triggers.is_empty() {
            parts.push(format!("Triggers: {}", self.triggers.join("; ")));
        }

        if !self.action_triggers.is_empty() {
            parts.push(format!("Actions: {}", self.action_triggers.join(", ")));
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

## Core Concepts

### YAML Task Definitions
Define tasks declaratively with judge criteria.
"#;

        let doc = SkillSemanticDocument::extract("agent-eval", sample);
        assert_eq!(doc.name, "agent-eval");
        assert!(doc.intent.contains("Head-to-head comparison"));
        assert!(!doc.triggers.is_empty());
        assert!(!doc.micro_rules.is_empty());
        assert!(doc.action_triggers.contains(&"eval".to_string()) || doc.action_triggers.contains(&"agent".to_string()));

        let passage = doc.to_passage(1500);
        assert!(passage.contains("Skill: agent-eval"));
        assert!(passage.contains("Intent:"));
        assert!(passage.contains("Key Rules:"));
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
        assert!(doc.file_patterns.contains(&"*.sql".to_string()));
        assert!(doc.action_triggers.contains(&"query".to_string()) || doc.action_triggers.contains(&"database".to_string()));
        assert!(!doc.micro_rules.is_empty());
    }
}
