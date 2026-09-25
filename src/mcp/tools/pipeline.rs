use serde_json::Value;

use crate::context::cache::project_snapshot;
use crate::mcp::state::ServerState;
use super::{detect_project_architecture, detect_project_path, ensure_indexed, ensure_not_cancelled};

pub fn infer_or_normalize_phase(raw_phase: Option<&str>, task: &str) -> String {
    let raw = raw_phase.unwrap_or("").trim().to_lowercase();
    match raw.as_str() {
        "build" | "implement" | "code" | "create" => return "build".to_string(),
        "test" | "verify" | "test_recheck" | "testing" => return "test".to_string(),
        "debug" | "fix" | "bugfix" | "patch" => return "fix".to_string(),
        "review" | "audit" | "proposal" => return "review".to_string(),
        "refactor" | "decompose" => return "refactor".to_string(),
        _ => {}
    }

    let task_lower = task.trim().to_lowercase();
    if task_lower.contains("plan")
        || task_lower.contains("kế hoạch")
        || task_lower.contains("design")
        || task_lower.contains("thiết kế")
        || task_lower.contains("architecture")
        || task_lower.contains("kiến trúc")
    {
        return "plan".to_string();
    }

    if task_lower.contains("fix")
        || task_lower.contains("bug")
        || task_lower.contains("crash")
        || task_lower.contains("sửa lỗi")
        || task_lower.contains("khắc phục")
        || task_lower.contains("vấn đề")
        || task_lower.contains("error")
        || task_lower.contains("issue")
    {
        return "fix".to_string();
    }

    if task_lower.contains("test")
        || task_lower.contains("kiểm thử")
        || task_lower.contains("verify")
        || task_lower.contains("assert")
        || task_lower.contains("kiem thu")
    {
        return "test".to_string();
    }

    if task_lower.contains("review")
        || task_lower.contains("audit")
        || task_lower.contains("đánh giá")
        || task_lower.contains("soát code")
        || task_lower.contains("soát lỗi")
    {
        return "review".to_string();
    }

    if task_lower.contains("refactor")
        || task_lower.contains("tách file")
        || task_lower.contains("decompose")
        || task_lower.contains("clean up")
        || task_lower.contains("tối ưu")
        || task_lower.contains("tái cấu trúc")
    {
        return "refactor".to_string();
    }

    if task_lower.contains("build")
        || task_lower.contains("implement")
        || task_lower.contains("triển khai")
        || task_lower.contains("phát triển")
    {
        return "build".to_string();
    }

    if !raw.is_empty() {
        raw
    } else {
        "plan".to_string()
    }
}

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let raw_task = arguments
        .get("task")
        .and_then(|t| t.as_str())
        .unwrap_or("general task");
    let task = if raw_task.trim().is_empty() {
        "general task"
    } else {
        raw_task.trim()
    };
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);
    state.update_project_path(&proj_path);
    let raw_phase = arguments
        .get("phase")
        .and_then(|p| p.as_str());
    let (phase, display_phase) = match raw_phase {
        Some("plan") | None => {
            let inferred = infer_or_normalize_phase(raw_phase, task);
            let disp = inferred.clone();
            (inferred, disp)
        }
        Some(explicit) => {
            let normalized = infer_or_normalize_phase(raw_phase, task);
            (normalized, explicit.to_string())
        }
    };

    // Record active phase and auto-reset approval state when starting a new planning phase
    let is_same_task = state.user_intent_summary.as_deref() == Some(task);
    let is_approval_msg = state.process_user_message(task);
    state.active_phase = Some(phase.clone());
    if is_approval_msg {
        state.workflow_stage = "Build".to_string();
        state.plan_approved = true;
        state.edit_authorized = true;
        let _ = state.save_to_dir(&proj_path);
        tracing::info!("Detected plan approval in task pipeline; transitioned to Build stage with plan_approved=true.");
    } else if phase == "plan" {
        // Only reset approval if this is a different task or if not already in Build/Test_Recheck
        if !is_same_task || (state.workflow_stage != "Build" && state.workflow_stage != "Test_Recheck") {
            state.workflow_stage = "Plan".to_string();
            state.plan_approved = false;
            state.edit_authorized = false;
            state.verification_passed = false;
            state.verification_command = None;
            state.expected_output_keyword = None;
            let _ = state.save_to_dir(&proj_path);
            tracing::info!(
                "Reset workflow stage to 'Plan' and plan_approved to false for new task pipeline execution."
            );
        }
    } else if (phase == "build" || phase == "fix" || phase == "refactor") && state.plan_approved {
        state.workflow_stage = "Build".to_string();
        state.edit_authorized = true;
    }
    state.user_intent_summary = Some(task.to_string());
    state.active_task = Some(task.to_string());

    // Fast-path file count: use existing code_graph.db count if present, else snapshot
    let file_count = {
        let db_path = proj_path.join(".agent-context").join("code_graph.db");
        if db_path.exists() {
            if let Ok(db) = crate::context::db::CodeGraphDb::open_read_only(&db_path) {
                db.conn
                    .query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, i64>(0))
                    .map(|c| c as usize)
                    .unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        }
    };
    let file_count = if file_count > 0 {
        file_count
    } else {
        project_snapshot(&proj_path).files.len()
    };
    ensure_not_cancelled(state)?;
    let _ = ensure_indexed(&proj_path);

    let detected_arch = detect_project_architecture(&proj_path);
    state.active_architecture_pattern = Some(detected_arch.clone());

    let core_rules_checklist = crate::catalog::rules::get_phase_rules(&phase);

    let dynamic_blueprint = crate::catalog::blueprint::generate_dynamic_blueprint(&proj_path, task, &detected_arch);
    let relevant_learnings = crate::mcp::learnings::get_semantic_relevant_learnings(&proj_path, task, 3, 0.82);

    let learnings_section = if relevant_learnings.is_empty() {
        String::new()
    } else {
        format!("\n\n## Project Memorized Learnings\n{}", relevant_learnings.join("\n"))
    };

    let blueprint_section = if dynamic_blueprint.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n## Dynamic Split Blueprint\n{}", dynamic_blueprint)
    };

    let next_step_prompt = if phase == "plan" {
        "-> NEXT STEP: Call `guidance(operation=\"search\", task=\"...\")` FIRST to discover and propose required domain skills. When skills are proposed, ask user via `ask_question` before invoking `select_skills(skills=[...], user_confirmed=true)`. If no matching skills are found, proceed directly to task planning or codebase inspection (`project_context`) without asking."
    } else if phase == "build" || phase == "fix" || phase == "refactor" {
        "-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. When ready to write code, obtain authorization via `workflow_gate(action=\"authorize_edit\", ...)`."
    } else if phase == "test" {
        "-> NEXT STEP: Run verification commands with `guidance(operation=\"verify\", verification_command=\"...\", expected_output_keyword=\"...\")`."
    } else if phase == "review" {
        "-> NEXT STEP: Inspect changes with `project_context(operation=\"read\")` and ensure compliance with 300 LOC limits."
    } else {
        "-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. Otherwise, proceed with task planning."
    };

    let indexing_suffix = if crate::context::graph_rag::jit_sync::is_indexing(&proj_path) {
        ", background GraphRAG indexing active"
    } else {
        ""
    };

    state.record_call(800, 300);
    Ok(format!(
        "# Task Pipeline Activated\n\nTask: {}\nActive Phase: {}\nProject: {} ({} files scanned{}){}{}\n\n## Architecture Guidance\n- Active Pattern: {}\n- Enforce: Create thin dispatcher main + sub-module files from line 1 (Upfront Architecture, 300 LOC Cap)\n\n{}\n\nPriority Gate: PASSED\nStatus: Ready for execution.\n\n{}",
        task,
        display_phase,
        proj_path.display(),
        file_count,
        indexing_suffix,
        learnings_section,
        blueprint_section,
        detected_arch,
        core_rules_checklist,
        next_step_prompt
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_or_normalize_phase() {
        assert_eq!(infer_or_normalize_phase(Some("implement"), "any task"), "build");
        assert_eq!(infer_or_normalize_phase(Some("debug"), "any task"), "fix");
        assert_eq!(infer_or_normalize_phase(Some("verify"), "any task"), "test");
        assert_eq!(infer_or_normalize_phase(Some("audit"), "any task"), "review");

        // Auto inference from task when phase is plan or empty
        assert_eq!(infer_or_normalize_phase(Some("plan"), "Fix crash on startup"), "fix");
        assert_eq!(infer_or_normalize_phase(Some("plan"), "Sửa lỗi crash"), "fix");
        assert_eq!(infer_or_normalize_phase(Some("plan"), "Write unit test for auth"), "test");
        assert_eq!(infer_or_normalize_phase(Some("plan"), "Code review for security"), "review");
        assert_eq!(infer_or_normalize_phase(Some("plan"), "Refactor scanner into sub-modules"), "refactor");
        assert_eq!(infer_or_normalize_phase(Some("plan"), "Triển khai tính năng mới"), "build");

        // Explicit plan in task preserves plan
        assert_eq!(infer_or_normalize_phase(Some("plan"), "Lên kế hoạch sửa lỗi crash"), "plan");
        assert_eq!(infer_or_normalize_phase(Some("plan"), "Design architecture for payment"), "plan");
        assert_eq!(infer_or_normalize_phase(Some("plan"), "general planning task"), "plan");
    }
}