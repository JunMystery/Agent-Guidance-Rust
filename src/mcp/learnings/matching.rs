use anyhow::Result;
use std::fs;
use std::path::Path;
use tracing::info;

use crate::mcp::state::ServerState;
use super::{LearningItem, parse_learnings_file, handoff_file_path};

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
    if let Some(model) = crate::ml::embeddings::try_cached_model() {
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

