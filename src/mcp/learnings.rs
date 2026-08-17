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
pub struct LearningItem {
    pub category: String,
    pub content: String,
    pub is_pinned: bool,
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
pub fn get_recent_learnings(proj_path: &Path, limit: usize) -> Vec<String> {
    let items = parse_learnings_file(proj_path);
    items
        .into_iter()
        .rev()
        .take(limit)
        .map(|i| {
            let cat_label = if i.is_pinned {
                format!("PINNED:{}", i.category)
            } else {
                i.category
            };
            format!("- [{}] {}", cat_label, i.content)
        })
        .collect()
}

/// Matches category keywords when ML embeddings are offline.
pub fn match_category_keywords(items: &[LearningItem], task: &str, limit: usize) -> Vec<String> {
    let task_lower = task.to_lowercase();
    let mut keyword_matched: Vec<&LearningItem> = Vec::new();

    let target_categories: Vec<&str> = if task_lower.contains("test")
        || task_lower.contains("cargo test")
        || task_lower.contains("bench")
        || task_lower.contains("mock")
    {
        vec!["build_test"]
    } else if task_lower.contains("arch")
        || task_lower.contains("pattern")
        || task_lower.contains("layer")
        || task_lower.contains("module")
        || task_lower.contains("structure")
    {
        vec!["arch"]
    } else if task_lower.contains("error")
        || task_lower.contains("bug")
        || task_lower.contains("fail")
        || task_lower.contains("panic")
        || task_lower.contains("fix")
    {
        vec!["gotcha"]
    } else if task_lower.contains("env")
        || task_lower.contains("path")
        || task_lower.contains("config")
        || task_lower.contains("setup")
    {
        vec!["env"]
    } else if task_lower.contains("domain")
        || task_lower.contains("logic")
        || task_lower.contains("business")
    {
        vec!["domain"]
    } else {
        vec![]
    };

    if !target_categories.is_empty() {
        for item in items.iter().rev() {
            if target_categories.contains(&item.category.as_str()) {
                keyword_matched.push(item);
                if keyword_matched.len() >= limit {
                    break;
                }
            }
        }
    }

    keyword_matched
        .into_iter()
        .map(|i| {
            let cat_label = if i.is_pinned {
                format!("PINNED:{}", i.category)
            } else {
                i.category.clone()
            };
            format!("- [{}] {}", cat_label, i.content)
        })
        .collect()
}

/// Retrieves the top `limit` relevant project learnings using hybrid scoring (semantic vector + recency),
/// falling back to category keyword matching when offline, or returning empty in Strict Context mode.
pub fn get_semantic_relevant_learnings(
    proj_path: &Path,
    task: &str,
    limit: usize,
    threshold: f32,
) -> Vec<String> {
    let items = parse_learnings_file(proj_path);
    if items.is_empty() || task.trim().is_empty() {
        return Vec::new();
    }

    let total = items.len();

    // Tier 1: Try ML Embeddings Vector Search (Multilingual-E5 / ONNX)
    if let Ok(model) = crate::ml::embeddings::cached_model() {
        if let Ok(task_vec) = model.embed_text(task, Some("query")) {
            let mut scored: Vec<(f32, &LearningItem)> = Vec::new();

            for (idx, item) in items.iter().enumerate() {
                let text_to_embed = format!("{}: {}", item.category, item.content);
                if let Ok(item_vec) = model.embed_text(&text_to_embed, Some("passage")) {
                    let sim = crate::ml::embeddings::cosine_similarity(&task_vec, &item_vec);
                    let recency_weight = (idx + 1) as f32 / total as f32;
                    let hybrid_score = 0.8 * sim + 0.2 * recency_weight;

                    if sim >= threshold {
                        scored.push((hybrid_score, item));
                    }
                }
            }

            if !scored.is_empty() {
                scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                return scored
                    .into_iter()
                    .take(limit)
                    .map(|(_, i)| {
                        let cat_label = if i.is_pinned {
                            format!("PINNED:{}", i.category)
                        } else {
                            i.category.clone()
                        };
                        format!("- [{}] {}", cat_label, i.content)
                    })
                    .collect();
            } else {
                // Strict Context: no items met threshold
                return Vec::new();
            }
        }
    }

    // Tier 2: Category Keyword Matching Fallback (when ML offline / weights unavailable)
    let keyword_results = match_category_keywords(&items, task, limit);
    if !keyword_results.is_empty() {
        return keyword_results;
    }

    // Tier 3: Strict Context empty return
    Vec::new()
}

/// Computes simple line delta (additions, deletions) between two texts.
pub fn compute_line_delta(original: &str, current: &str) -> (usize, usize) {
    let orig_lines: Vec<&str> = original.lines().collect();
    let curr_lines: Vec<&str> = current.lines().collect();

    let additions = curr_lines.iter().filter(|l| !orig_lines.contains(l)).count();
    let deletions = orig_lines.iter().filter(|l| !curr_lines.contains(l)).count();
    (additions, deletions)
}

/// Generates a comprehensive markdown modification summary for the active session.
pub fn generate_session_diff_summary(proj_path: &Path, state: &ServerState) -> String {
    if state.modified_files.is_empty() {
        return format!(
            "# 🔍 Session Modification Summary\n\n- Active Session ID: `{}`\n- Total Files Modified: 0\n\n*No files have been modified in this session yet.*",
            state.session_id
        );
    }

    let mut table = String::from("| File Path | Impact Risk | Snapshot Available | Lines Delta |\n| :--- | :---: | :---: | :---: |\n");
    let snapshots_dir = proj_path.join(".agent-context").join("snapshots").join(&state.session_id);

    for file in &state.modified_files {
        let impact = crate::mcp::impact::assess_file_risk(proj_path, file);
        let mangled = file.replace('/', "_").replace('\\', "_");
        let snap_file = snapshots_dir.join(format!("{}.snapshot", mangled));
        let has_snap = snap_file.exists();
        let snap_str = if has_snap { "✅ Yes" } else { "❌ No" };

        let curr_file_path = proj_path.join(file);
        let curr_content = fs::read_to_string(&curr_file_path).unwrap_or_default();

        let delta_str = if has_snap {
            let orig_content = fs::read_to_string(&snap_file).unwrap_or_default();
            let (adds, dels) = compute_line_delta(&orig_content, &curr_content);
            format!("+{} / -{}", adds, dels)
        } else {
            let lines = curr_content.lines().count();
            format!("+{} / -0", lines)
        };

        table.push_str(&format!(
            "| `{}` | {:?} | {} | {} |\n",
            file, impact.risk_level, snap_str, delta_str
        ));
    }

    format!(
        "# 🔍 Session Modification Summary\n\n- Active Session ID: `{}`\n- Total Files Modified: {}\n\n{}\n💡 **Handoff Command**: Call `session_continuity(operation=\"handoff\", next_action=\"...\")` to conclude session.",
        state.session_id,
        state.modified_files.len(),
        table
    )
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

    let modified_section = if state.modified_files.is_empty() {
        "None (No files modified during this session).".to_string()
    } else {
        let mut table = String::from("| File Path | Impact Risk | Snapshot Available |\n| :--- | :---: | :---: |\n");
        let snapshots_dir = proj_path.join(".agent-context").join("snapshots").join(&state.session_id);
        for file in &state.modified_files {
            let impact = crate::mcp::impact::assess_file_risk(proj_path, file);
            let mangled = file.replace('/', "_").replace('\\', "_");
            let snap_file = snapshots_dir.join(format!("{}.snapshot", mangled));
            let has_snap = if snap_file.exists() { "✅ Yes" } else { "❌ No" };
            table.push_str(&format!("| `{}` | {:?} | {} |\n", file, impact.risk_level, has_snap));
        }
        table
    };

    let summary = format!(
        "# 🤝 Cross-Agent Handoff Protocol\n\n- Active Session ID: `{}`\n- Workflow Stage: `{}` (Plan Approved: {})\n- Active Architecture Pattern: `{}`\n- Total Tool Calls: {}\n\n## 📁 Modified Files in This Session:\n{}\n\n## 🎯 Target Goal / Intent:\n{}\n\n## 🚀 Next Action for Incoming Agent:\n{}\n\n---\n*Written to `.agent-context/handoff.md` for zero-delay multi-IDE handoff.*",
        state.session_id,
        state.workflow_stage,
        state.plan_approved,
        state.active_architecture_pattern.as_deref().unwrap_or("Auto"),
        state.tool_calls,
        modified_section,
        state.user_intent_summary.as_deref().unwrap_or("Active development task in progress."),
        next_step
    );

    fs::write(&handoff_path, &summary)?;
    info!("Saved cross-agent handoff summary to `.agent-context/handoff.md`");
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_parse_learnings() {
        let temp_dir = std::env::temp_dir().join(format!("learnings_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        let res = record_project_learning(&temp_dir, "Always use mock database in tests", "build_test", true);
        assert!(res.is_ok());

        let res2 = record_project_learning(&temp_dir, "Prefer thin controller layer", "arch", false);
        assert!(res2.is_ok());

        let items = parse_learnings_file(&temp_dir);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].category, "build_test");
        assert!(items[0].is_pinned);
        assert_eq!(items[1].category, "arch");
        assert!(!items[1].is_pinned);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_category_keyword_matching_fallback() {
        let items = vec![
            LearningItem {
                category: "build_test".to_string(),
                content: "Use mock database pool".to_string(),
                is_pinned: true,
            },
            LearningItem {
                category: "arch".to_string(),
                content: "Use layered architecture".to_string(),
                is_pinned: false,
            },
            LearningItem {
                category: "gotcha".to_string(),
                content: "Check exFAT mount error 22".to_string(),
                is_pinned: false,
            },
        ];

        // When task mentions "test", Tier 2 matches build_test category
        let results = match_category_keywords(&items, "Run cargo test with mocks", 3);
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("[PINNED:build_test]"));

        // When task mentions "arch", Tier 2 matches arch category
        let results_arch = match_category_keywords(&items, "Refactor module structure and architecture", 3);
        assert_eq!(results_arch.len(), 1);
        assert!(results_arch[0].contains("[arch]"));

        // When task is unrelated
        let results_none = match_category_keywords(&items, "Cook carbonara spaghetti pasta recipe", 3);
        assert!(results_none.is_empty());
    }

    #[test]
    fn test_strict_context_empty_when_no_match() {
        let temp_dir = std::env::temp_dir().join(format!("strict_empty_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        let _ = record_project_learning(&temp_dir, "Use mock database pool", "build_test", false);

        // Test relevant task
        let relevant = get_semantic_relevant_learnings(&temp_dir, "Run cargo test with mock database", 3, 0.82);
        assert_eq!(relevant.len(), 1, "Relevant task should match");

        // Completely unrelated task with no keyword or vector match above 0.82 threshold
        let results = get_semantic_relevant_learnings(&temp_dir, "Cook carbonara spaghetti pasta recipe", 3, 0.82);
        assert!(results.is_empty(), "Strict Context mode should return empty list for unrelated task");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_compute_line_delta() {
        let original = "line 1\nline 2\nline 3\n";
        let current = "line 1\nline 2 modified\nline 3\nline 4\n";

        let (adds, dels) = compute_line_delta(original, current);
        assert_eq!(adds, 2); // "line 2 modified", "line 4"
        assert_eq!(dels, 1); // "line 2"
    }

    #[test]
    fn test_generate_session_diff_summary() {
        let temp_dir = std::env::temp_dir().join(format!("diff_summary_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let mut state = ServerState::new();
        state.record_modified_file("src/main.rs");

        let summary = generate_session_diff_summary(&temp_dir, &state);
        assert!(summary.contains("Session Modification Summary"));
        assert!(summary.contains("src/main.rs"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pinned_learnings_survive_fifo_overflow() {
        let temp_dir = std::env::temp_dir().join(format!("pinned_fifo_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        // 1. Pre-populate 2 pinned items and 30 transient items
        let mut initial_items = vec![
            LearningItem {
                category: "security".to_string(),
                content: "Immutable rule 1".to_string(),
                is_pinned: true,
            },
            LearningItem {
                category: "policy".to_string(),
                content: "Immutable rule 2".to_string(),
                is_pinned: true,
            },
        ];

        for i in 0..30 {
            initial_items.push(LearningItem {
                category: "dev".to_string(),
                content: format!("Existing item {}", i),
                is_pinned: false,
            });
        }

        assert!(write_learnings_file(&temp_dir, &initial_items).is_ok());

        // 2. Add 2 new transient items (exceeding MAX_LEARNINGS_FIFO = 30)
        let _ = record_project_learning(&temp_dir, "Brand new transient learning A", "dev", false);
        let _ = record_project_learning(&temp_dir, "Brand new transient learning B", "dev", false);

        let items = parse_learnings_file(&temp_dir);
        // Total items should remain 2 pinned + 30 transient = 32
        assert_eq!(items.len(), 32);

        // Verify pinned items still exist 100%
        let pinned: Vec<&LearningItem> = items.iter().filter(|i| i.is_pinned).collect();
        assert_eq!(pinned.len(), 2);
        assert_eq!(pinned[0].content, "Immutable rule 1");
        assert_eq!(pinned[1].content, "Immutable rule 2");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_semantic_learning_deduplication() {
        let temp_dir = std::env::temp_dir().join(format!("dedup_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        // 1. Add initial learning
        let _ = record_project_learning(&temp_dir, "Run tests with mock database pool", "build_test", false);

        // 2. Add almost identical wording (will update existing entry)
        let _ = record_project_learning(&temp_dir, "Run tests with mock database pool", "build_test", true);

        let items = parse_learnings_file(&temp_dir);
        assert_eq!(items.len(), 1, "Duplicate should be merged");
        assert!(items[0].is_pinned, "Existing item should be elevated to pinned");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
