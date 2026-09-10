use anyhow::Result;
use std::fs;
use std::path::Path;
use tracing::info;

use crate::mcp::templates::{
    AGENT_GUIDANCE_SKILL_TAG_END, AGENT_GUIDANCE_SKILL_TAG_START,
    AGENT_GUIDANCE_TAG_END, AGENT_GUIDANCE_TAG_START,
};

pub(crate) fn remove_global_rules(home: &Path) -> Result<()> {
    let targets = vec![
        home.join(".gemini").join("GEMINI.md"),
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
