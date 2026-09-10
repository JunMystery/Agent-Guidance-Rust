use super::document::SkillSemanticDocument;

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
