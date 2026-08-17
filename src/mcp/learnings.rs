use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::mcp::state::ServerState;

const MAX_LEARNINGS_FIFO: usize = 30;

fn learnings_file_path(proj_path: &Path) -> PathBuf {
    proj_path.join(".agent-context").join("learnings.md")
}

fn handoff_file_path(proj_path: &Path) -> PathBuf {
    proj_path.join(".agent-context").join("handoff.md")
}

#[derive(Debug, Clone)]
struct LearningItem {
    category: String,
    content: String,
}

fn parse_learnings_file(proj_path: &Path) -> Vec<LearningItem> {
    let path = learnings_file_path(proj_path);
    if !path.exists() {
        return Vec::new();
    }

    let Ok(content) = fs::read_to_string(&path) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    let mut current_cat = "general".to_string();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## [") && trimmed.ends_with(']') {
            current_cat = trimmed[4..trimmed.len() - 1].to_string();
        } else if trimmed.starts_with("- ") {
            let entry = trimmed[2..].trim().to_string();
            if !entry.is_empty() {
                items.push(LearningItem {
                    category: current_cat.clone(),
                    content: entry,
                });
            }
        }
    }

    items
}

fn write_learnings_file(proj_path: &Path, items: &[LearningItem]) -> Result<()> {
    let path = learnings_file_path(proj_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut output = String::from("# Project Memorized Learnings\n\n> Shared operational memory and distilled project rules.\n\n");
    let mut current_cat = String::new();

    for item in items {
        if item.category != current_cat {
            current_cat = item.category.clone();
            output.push_str(&format!("## [{}]\n", current_cat));
        }
        output.push_str(&format!("- {}\n", item.content));
    }

    fs::write(&path, output)?;
    Ok(())
}

/// Records a new project learning item with a specific category under FIFO (capped at 30).
pub fn record_project_learning(proj_path: &Path, learning: &str, category: &str) -> Result<String> {
    let clean_learning = learning.trim();
    if clean_learning.is_empty() {
        return Ok("Empty learning string ignored.".to_string());
    }

    let clean_cat = if category.trim().is_empty() {
        "general"
    } else {
        category.trim()
    };

    let mut items = parse_learnings_file(proj_path);

    // Deduplicate identical content
    if !items.iter().any(|i| i.content == clean_learning) {
        items.push(LearningItem {
            category: clean_cat.to_string(),
            content: clean_learning.to_string(),
        });

        // Enforce FIFO cap of 30 items
        if items.len() > MAX_LEARNINGS_FIFO {
            let drop_count = items.len() - MAX_LEARNINGS_FIFO;
            items.drain(0..drop_count);
        }

        write_learnings_file(proj_path, &items)?;
        info!("Saved new project learning in category '{}'", clean_cat);
    }

    Ok(format!(
        "# Project Learning Saved ✓\n\n- Category: `{}`\n- Learning: {}\n- Total Memorized Items: {}\n- File: `.agent-context/learnings.md`",
        clean_cat, clean_learning, items.len()
    ))
}

/// Reads the most recent project learnings up to `limit` entries for context injection.
pub fn get_recent_learnings(proj_path: &Path, limit: usize) -> Vec<String> {
    let items = parse_learnings_file(proj_path);
    items
        .into_iter()
        .rev()
        .take(limit)
        .map(|i| format!("- [{}] {}", i.category, i.content))
        .collect()
}

/// Writes and returns a concise Cross-Agent Handoff Summary document.
pub fn write_handoff_summary(proj_path: &Path, state: &ServerState, next_action: &str) -> Result<String> {
    let handoff_path = handoff_file_path(proj_path);
    if let Some(parent) = handoff_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let next_step = if next_action.trim().is_empty() {
        "Inspect recent changes via `project_context(operation=\"read\")` and continue task execution."
    } else {
        next_action.trim()
    };

    let summary = format!(
        "# 🤝 Cross-Agent Handoff Protocol\n\n- Active Session ID: `{}`\n- Workflow Stage: `{}` (Plan Approved: {})\n- Active Architecture Pattern: `{}`\n- Total Tool Calls: {}\n\n## 🎯 Target Goal / Intent:\n{}\n\n## 🚀 Next Action for Incoming Agent:\n{}\n\n---\n*Written to `.agent-context/handoff.md` for zero-delay multi-IDE handoff.*",
        state.session_id,
        state.workflow_stage,
        state.plan_approved,
        state.active_architecture_pattern.as_deref().unwrap_or("Auto"),
        state.tool_calls,
        state.user_intent_summary.as_deref().unwrap_or("Active development task in progress."),
        next_step
    );

    fs::write(&handoff_path, &summary)?;
    info!("Saved cross-agent handoff summary to `.agent-context/handoff.md`");
    Ok(summary)
}
