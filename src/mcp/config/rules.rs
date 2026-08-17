use anyhow::Result;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::mcp::templates::*;

pub(crate) fn remove_global_rules(home: &Path) -> Result<()> {
    let targets = vec![
        home.join(".gemini").join("config").join("AGENTS.md"),
        home.join(".config").join("opencode").join("AGENTS.md"),
        home.join(".claude").join("CLAUDE.md"),
        home.join(".codex").join("AGENTS.md"),
        home.join(".codeium").join("windsurf").join("AGENTS.md"),
    ];
    for path in &targets {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                let cleaned = strip_tagged_section(
                    &content,
                    AGENT_GUIDANCE_TAG_START,
                    AGENT_GUIDANCE_TAG_END,
                );
                if cleaned != content {
                    fs::write(path, cleaned)?;
                    info!("Cleaned rules from: {}", path.display());
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn remove_skills_enforcer(home: &Path) -> Result<()> {
    let targets = vec![
        home.join(".claude")
            .join("skills")
            .join("agent-guidance")
            .join("SKILL.md"),
        home.join(".config")
            .join("opencode")
            .join("skills")
            .join("agent-guidance")
            .join("SKILL.md"),
        home.join(".agents")
            .join("skills")
            .join("agent-guidance")
            .join("SKILL.md"),
        home.join(".codex")
            .join("skills")
            .join("agent-guidance")
            .join("SKILL.md"),
        home.join(".codeium")
            .join("windsurf")
            .join("skills")
            .join("agent-guidance")
            .join("SKILL.md"),
    ];
    for path in &targets {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                let cleaned = strip_tagged_section(
                    &content,
                    AGENT_GUIDANCE_SKILL_TAG_START,
                    AGENT_GUIDANCE_SKILL_TAG_END,
                );
                if cleaned.trim().is_empty() {
                    let _ = fs::remove_file(path);
                    info!("Removed skill enforcer: {}", path.display());
                } else if cleaned != content {
                    fs::write(path, cleaned)?;
                    info!("Cleaned skill enforcer: {}", path.display());
                }
            }
        }
    }
    Ok(())
}

fn strip_tagged_section(content: &str, start_tag: &str, end_tag: &str) -> String {
    if let (Some(start_idx), Some(end_idx)) = (content.find(start_tag), content.find(end_tag)) {
        let before = content[..start_idx].trim_end();
        let after = content[end_idx + end_tag.len()..].trim_start();
        let mut res = String::new();
        res.push_str(before);
        if !before.is_empty() && !after.is_empty() {
            res.push('\n');
        }
        res.push_str(after);
        res
    } else {
        content.to_string()
    }
}

// ---- Verification Helpers ----

pub(crate) fn configure_global_rules(home: &Path) -> Result<()> {
    let targets = vec![
        (
            "Gemini/Antigravity",
            home.join(".gemini").join("config").join("AGENTS.md"),
        ),
        (
            "OpenCode",
            home.join(".config").join("opencode").join("AGENTS.md"),
        ),
        (
            "Claude Code Compatibility",
            home.join(".claude").join("CLAUDE.md"),
        ),
        ("ChatGPT/Codex", home.join(".codex").join("AGENTS.md")),
        (
            "Windsurf",
            home.join(".codeium").join("windsurf").join("AGENTS.md"),
        ),
    ];

    for (_name, path) in targets {
        let content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        let new_content = replace_or_append_tagged_section(
            &content,
            AGENT_GUIDANCE_TAG_START,
            AGENT_GUIDANCE_TAG_END,
            crate::mcp::templates::AGENT_RULES_BLOCK.trim(),
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, new_content)?;
    }

    Ok(())
}

pub(crate) fn configure_skills_enforcer(home: &Path) -> Result<()> {
    let global_targets = vec![
        (
            "Claude Code Global",
            home.join(".claude")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
        ),
        (
            "OpenCode Global",
            home.join(".config")
                .join("opencode")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
        ),
        (
            "Cline/Roo-Code Global",
            home.join(".agents")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
        ),
        (
            "ChatGPT/Codex Global",
            home.join(".codex")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
        ),
        (
            "Windsurf Global",
            home.join(".codeium")
                .join("windsurf")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
        ),
    ];

    for (_name, path) in global_targets {
        let content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        let new_content = replace_or_append_tagged_section(
            &content,
            AGENT_GUIDANCE_SKILL_TAG_START,
            AGENT_GUIDANCE_SKILL_TAG_END,
            crate::mcp::templates::ENFORCER_SKILL_CONTENT.trim(),
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, new_content)?;
    }

    Ok(())
}

pub fn replace_or_append_tagged_section(
    content: &str,
    start_tag: &str,
    end_tag: &str,
    new_section: &str,
) -> String {
    if let (Some(start_idx), Some(end_idx)) = (content.find(start_tag), content.find(end_tag))
        && start_idx < end_idx
    {
        let before = content[..start_idx].trim_end();
        let after = content[end_idx + end_tag.len()..].trim_start();
        let mut res = String::new();
        res.push_str(before);
        res.push('\n');
        res.push_str(new_section.trim());
        res.push('\n');
        res.push_str(after);
        return res;
    }

    if content.trim().is_empty() {
        new_section.trim().to_string()
    } else {
        let mut res = String::new();
        res.push_str(content.trim());
        res.push('\n');
        res.push('\n');
        res.push_str(new_section.trim());
        res
    }
}
