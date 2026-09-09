use serde_json::Value;

use crate::context::cache::project_snapshot;
use crate::mcp::state::ServerState;
use super::{detect_project_architecture, detect_project_path, ensure_not_cancelled};

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
    let phase = arguments
        .get("phase")
        .and_then(|p| p.as_str())
        .unwrap_or("plan");

    // Record active phase and auto-reset approval state when starting a new planning phase
    let is_same_task = state.user_intent_summary.as_deref() == Some(task);
    let is_approval_msg = state.process_user_message(task);
    state.active_phase = Some(phase.to_string());
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
    } else if phase == "build" && state.plan_approved {
        state.workflow_stage = "Build".to_string();
        state.edit_authorized = true;
    }
    state.user_intent_summary = Some(task.to_string());

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

    let detected_arch = detect_project_architecture(&proj_path);
    state.active_architecture_pattern = Some(detected_arch.clone());

    let core_rules_checklist = crate::catalog::rules::get_phase_rules(phase);

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

    let next_step_prompt = "-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. Otherwise, proceed with task planning.";

    state.record_call(800, 300);
    Ok(format!(
        "# Task Pipeline Activated\n\nTask: {}\nActive Phase: {}\nProject: {} ({} files scanned){}{}\n\n## Architecture Guidance\n- Active Pattern: {}\n- Enforce: Create thin dispatcher main + sub-module files from line 1 (Upfront Architecture, 300 LOC Cap)\n\n{}\n\nPriority Gate: PASSED\nStatus: Ready for execution.\n\n{}",
        task,
        phase,
        proj_path.display(),
        file_count,
        learnings_section,
        blueprint_section,
        detected_arch,
        core_rules_checklist,
        next_step_prompt
    ))
}