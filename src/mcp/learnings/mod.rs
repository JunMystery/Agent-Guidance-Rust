use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

pub mod matching;
pub use matching::{
    compute_line_delta, generate_session_diff_summary,
    get_recent_learnings, get_semantic_relevant_learnings,
    match_category_keywords, write_handoff_summary,
};
pub use crate::mcp::state::ServerState;

const MAX_LEARNINGS_FIFO: usize = 30;

pub(crate) fn learnings_file_path(proj_path: &Path) -> PathBuf {
    proj_path.join(".agent-context").join("learnings.md")
}

pub(crate) fn handoff_file_path(proj_path: &Path) -> PathBuf {
    proj_path.join(".agent-context").join("handoff.md")
}

#[derive(Debug, Clone)]
pub struct LearningItem {
    pub category: String,
    pub content: String,
    pub is_pinned: bool,
}

pub(crate) fn parse_learnings_file(proj_path: &Path) -> Vec<LearningItem> {
    let path = learnings_file_path(proj_path);
    if !path.exists() {
        return Vec::new();
    }

    let Ok(content) = fs::read_to_string(&path) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    let mut current_cat = "general".to_string();
    let mut current_pinned = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## [") && trimmed.ends_with(']') {
            let header = &trimmed[4..trimmed.len() - 1];
            if header.to_uppercase().starts_with("PINNED:") {
                current_pinned = true;
                current_cat = header[7..].to_string();
            } else {
                current_pinned = false;
                current_cat = header.to_string();
            }
        } else if trimmed.starts_with("- ") {
            let entry = trimmed[2..].trim().to_string();
            if !entry.is_empty() {
                items.push(LearningItem {
                    category: current_cat.clone(),
                    content: entry,
                    is_pinned: current_pinned,
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

    let pinned_items: Vec<&LearningItem> = items.iter().filter(|i| i.is_pinned).collect();
    let transient_items: Vec<&LearningItem> = items.iter().filter(|i| !i.is_pinned).collect();

    let mut current_cat = String::new();
    for item in &pinned_items {
        let cat_key = format!("PINNED:{}", item.category);
        if cat_key != current_cat {
            current_cat = cat_key.clone();
            output.push_str(&format!("## [{}]\n", current_cat));
        }
        output.push_str(&format!("- {}\n", item.content));
    }

    current_cat.clear();
    for item in &transient_items {
        if item.category != current_cat {
            current_cat = item.category.clone();
            output.push_str(&format!("## [{}]\n", current_cat));
        }
        output.push_str(&format!("- {}\n", item.content));
    }

    fs::write(&path, output)?;
    Ok(())
}

/// Records a new project learning item with Pinned support and Semantic Deduplication.
pub fn record_project_learning(
    proj_path: &Path,
    learning: &str,
    category: &str,
    is_pinned: bool,
) -> Result<String> {
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
    let mut duplicate_found = false;

    // 1. Exact match fast check
    if let Some(existing) = items
        .iter_mut()
        .find(|i| i.category == clean_cat && i.content.eq_ignore_ascii_case(clean_learning))
    {
        if is_pinned {
            existing.is_pinned = true;
        }
        duplicate_found = true;
    }

    // 2. Semantic Deduplication check via ML vector similarity (Cosine >= 0.92 for recent category items)
    if !duplicate_found {
        if let Ok(model) = crate::ml::embeddings::cached_model() {
            if let Ok(new_vec) = model.embed_text(clean_learning, Some("passage")) {
                for existing in items.iter_mut().rev().take(10) {
                    if existing.category == clean_cat {
                        let existing_text = format!("{}: {}", existing.category, existing.content);
                        if let Ok(ex_vec) = model.embed_text(&existing_text, Some("passage")) {
                            let sim = crate::ml::embeddings::cosine_similarity(&new_vec, &ex_vec);
                            if sim >= 0.92 {
                                existing.content = clean_learning.to_string();
                                if is_pinned {
                                    existing.is_pinned = true;
                                }
                                duplicate_found = true;
                                info!("Semantic duplicate learning updated (Cosine: {:.2}): '{}'", sim, clean_learning);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if !duplicate_found {
        items.push(LearningItem {
            category: clean_cat.to_string(),
            content: clean_learning.to_string(),
            is_pinned,
        });

        // Enforce FIFO cap of 30 items ONLY on non-pinned (transient) items
        let transient_count = items.iter().filter(|i| !i.is_pinned).count();
        if transient_count > MAX_LEARNINGS_FIFO {
            let mut drop_needed = transient_count - MAX_LEARNINGS_FIFO;
            items.retain(|i| {
                if !i.is_pinned && drop_needed > 0 {
                    drop_needed -= 1;
                    false
                } else {
                    true
                }
            });
        }
    }

    write_learnings_file(proj_path, &items)?;
    let pin_label = if is_pinned { " (PINNED 📌)" } else { "" };
    info!("Saved project learning in category '{}{}'", clean_cat, pin_label);

    Ok(format!(
        "# Project Learning Saved ✓\n\n- Category: `{}`{}\n- Learning: {}\n- Total Memorized Items: {}\n- File: `.agent-context/learnings.md`",
        clean_cat, pin_label, clean_learning, items.len()
    ))
}

/// Reads the most recent project learnings up to `limit` entries for context injection.

#[cfg(test)]
#[path = "../learnings_tests.rs"]
mod tests;

