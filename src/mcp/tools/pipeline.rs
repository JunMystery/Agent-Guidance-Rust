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
    state.active_phase = Some(phase.to_string());
    if phase == "plan" {
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

    let snapshot = project_snapshot(&proj_path);
    ensure_not_cancelled(state)?;
    let file_count = snapshot.files.len();

    let detected_arch = detect_project_architecture(&proj_path);
    state.active_architecture_pattern = Some(detected_arch.clone());

    let core_rules_checklist = crate::catalog::rules::get_phase_rules(phase);

    let dynamic_blueprint = crate::catalog::blueprint::generate_dynamic_blueprint(&proj_path, task, &detected_arch);
    let relevant_learnings = crate::mcp::learnings::get_semantic_relevant_learnings(&proj_path, task, 3, 0.82);

    let learnings_section = if relevant_learnings.is_empty() {
        String::new()
    } else {
        format!("\n\n## 💡 Project Memorized Learnings\n{}", relevant_learnings.join("\n"))
    };

    let blueprint_section = if dynamic_blueprint.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n## 📐 Dynamic Split Blueprint\n{}", dynamic_blueprint)
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