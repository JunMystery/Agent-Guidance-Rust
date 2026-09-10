use anyhow::Result;
use std::fs;
use std::path::Path;

pub(crate) use super::rules_cleaner::{remove_global_rules, remove_skills_enforcer};
use crate::mcp::templates::*;

pub(crate) fn configure_global_rules(home: &Path) -> Result<()> {
    let targets = vec![
        (
            "Gemini/Antigravity Root",
            home.join(".gemini").join("GEMINI.md"),
            TargetClient::Antigravity,
        ),
        (
            "Gemini/Antigravity Config",
            home.join(".gemini").join("config").join("AGENTS.md"),
            TargetClient::Antigravity,
        ),
        (
            "OpenCode",
            home.join(".config").join("opencode").join("AGENTS.md"),
            TargetClient::Generic,
        ),
        (
            "Claude Code Compatibility",
            home.join(".claude").join("CLAUDE.md"),
            TargetClient::ClaudeCode,
        ),
        ("ChatGPT/Codex", home.join(".codex").join("AGENTS.md"), TargetClient::ChatGptCodex),
        (
            "Cursor Global Rules",
            home.join(".cursor").join("rules").join("agent-guidance.mdc"),
            TargetClient::Cursor,
        ),
        (
            "Windsurf",
            home.join(".codeium").join("windsurf").join("AGENTS.md"),
            TargetClient::Generic,
        ),
    ];

    for (_name, path, client) in targets {
        let content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        let new_content = replace_or_append_tagged_section(
            &content,
            AGENT_GUIDANCE_TAG_START,
            AGENT_GUIDANCE_TAG_END,
            get_client_rules(client).trim(),
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
            TargetClient::ClaudeCode,
        ),
        (
            "OpenCode Global",
            home.join(".config")
                .join("opencode")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
            TargetClient::Generic,
        ),
        (
            "Cline/Roo-Code Global",
            home.join(".agents")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
            TargetClient::Generic,
        ),
        (
            "ChatGPT/Codex Global",
            home.join(".codex")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
            TargetClient::ChatGptCodex,
        ),
        (
            "Cursor Global",
            home.join(".cursor")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
            TargetClient::Cursor,
        ),
        (
            "Windsurf Global",
            home.join(".codeium")
                .join("windsurf")
                .join("skills")
                .join("agent-guidance")
                .join("SKILL.md"),
            TargetClient::Generic,
        ),
    ];

    for (_name, path, client) in global_targets {
        let content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        let new_content = replace_or_append_tagged_section(
            &content,
            AGENT_GUIDANCE_SKILL_TAG_START,
            AGENT_GUIDANCE_SKILL_TAG_END,
            get_client_enforcer_skill(client).trim(),
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

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
