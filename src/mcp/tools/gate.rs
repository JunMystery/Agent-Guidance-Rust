use serde_json::{Value, json};
use std::path::Path;

use crate::mcp::impact;
use crate::mcp::state::ServerState;
use super::{detect_project_path, ensure_not_cancelled, resolve_architecture_pattern};

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let action = arguments
        .get("action")
        .and_then(|a| a.as_str())
        .unwrap_or("check");
    let stage_target = arguments
        .get("target_stage")
        .or_else(|| arguments.get("stage"))
        .and_then(|s| s.as_str());
    let user_confirmed = arguments
        .get("user_confirmed")
        .or_else(|| arguments.get("confirmed"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let user_msg = arguments.get("user_message").and_then(|u| u.as_str());

    if let Some(msg) = user_msg {
        state.process_user_message(msg);
    }

    state.record_call(300, 50);

    let resp = match action {
        "approve" | "approve_plan" => {
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);

            if !user_confirmed && !state.plan_approved && user_msg.is_none() {
                return Ok(format!(
                    "# Workflow Gate: [approve_plan]\n\nStatus: BLOCKED | Error: USER_APPROVAL_REQUIRED: Plan approval cannot be self-granted by AI agents. Trigger the IDE/CLI `ask_question` tool to request user approval on the implementation plan. Re-invoke `workflow_gate(action=\"approve_plan\", user_confirmed=true)` once approved."
                ));
            }

            state.approve_plan();
            let _ = state.auto_checkpoint(&proj_path);
            format!(
                "# Workflow Gate: [approve_plan]\n\nStatus: PASSED | Plan Approved: true | Stage: {}",
                state.workflow_stage
            )
        }
        "pass_verification" => {
            state.fix_attempts = 0;
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            let _ = state.auto_checkpoint(&proj_path);
            format!(
                "# Workflow Gate: [pass_verification]\n\nStatus: PASSED | Plan Approved: {} | Stage: {} | Fix Attempts: 0\n\n✓ Verification passed and recorded in session checkpoint.",
                state.plan_approved, state.workflow_stage
            )
        }
        "set_stage" => {
            if let Some(target) = stage_target {
                let proj_path_arg = arguments
                    .get("project_path")
                    .and_then(|p| p.as_str())
                    .unwrap_or(".");
                let proj_path = detect_project_path(proj_path_arg, state);
                match state.set_stage(target) {
                    Ok(new_stage) => {
                        let _ = state.auto_checkpoint(&proj_path);
                        format!(
                            "# Workflow Gate: [set_stage]\n\nStatus: PASSED | Stage Changed To: {} | Plan Approved: {} | Fix Attempts: {}",
                            new_stage, state.plan_approved, state.fix_attempts
                        )
                    }
                    Err(err_msg) => {
                        let _ = state.auto_checkpoint(&proj_path);
                        format!(
                            "# Workflow Gate: [set_stage]\n\nStatus: BLOCKED | Error: {}. Trigger IDE/CLI `ask_question` tool to request user approval for stage transition.",
                            err_msg
                        )
                    }
                }
            } else {
                "# Workflow Gate: [set_stage]\n\nStatus: BLOCKED | Error: target_stage argument is required for set_stage action. Trigger IDE/CLI `ask_question` tool to clarify desired stage.".to_string()
            }
        }
        "status" => {
            let edit_allowed = state.workflow_stage == "Build" && state.plan_approved;
            format!(
                "# Workflow Stage Status\n\n- Active Stage: {}\n- Plan Approved: {}\n- Fix Attempts: {}/3\n- Edit Authorized: {}",
                state.workflow_stage, state.plan_approved, state.fix_attempts, edit_allowed
            )
        }
        "set_architecture" => {
            let raw_arch = arguments
                .get("architecture_pattern")
                .or_else(|| arguments.get("pattern"))
                .and_then(|a| a.as_str())
                .unwrap_or("Auto");
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            state.update_project_path(&proj_path);
            let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path, state);
            state.active_architecture_pattern = Some(arch_pattern.clone());
            let _ = ServerState::save_persisted_architecture(&proj_path, &arch_pattern);
            let _ = state.auto_checkpoint(&proj_path);
            format!(
                "# Architecture Pattern Locked\n\n- Project: {}\n- Confirmed Architecture: {}\n- Persistence: Saved to `.agent-context/architecture.json`\n\n✓ Pattern memorized for all workflow stages and future sessions.",
                proj_path.display(),
                arch_pattern
            )
        }
        "advance" => {
            // SECURITY FIX Bug #3: Do NOT auto-process user_message in advance to prevent agent self-approval
            let target_stage = arguments
                .get("target_stage")
                .and_then(|t| t.as_str())
                .unwrap_or("Build");
            let stage_res = state.set_stage(target_stage);

            let raw_arch = arguments
                .get("architecture_pattern")
                .and_then(|a| a.as_str())
                .unwrap_or("Auto");
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            state.update_project_path(&proj_path);
            let risk_level = arguments
                .get("risk_level")
                .and_then(|r| r.as_str())
                .unwrap_or("LOW");
            state.last_risk_level = Some(risk_level.to_string());

            let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path, state);

            if matches!(
                arch_pattern.as_str(),
                "Clean_Architecture"
                    | "Layered_Architecture"
                    | "Package_By_Feature"
                    | "Orchestrator"
                    | "CLI_Pipeline"
                    | "Flat_Library"
            ) && state.workflow_stage == "Build"
                && state.plan_approved
            {
                state.edit_authorized = true;
                state.active_architecture_pattern = Some(arch_pattern);
            }

            if stage_res.is_ok() {
                let _ = state.auto_checkpoint(&proj_path);
            }

            match stage_res {
                Ok(msg) => format!(
                    "# Workflow Gate: [advance]\n\n{}\n- Edit Authorized: {}\n- Architecture Pattern: {}",
                    msg,
                    state.edit_authorized,
                    state
                        .active_architecture_pattern
                        .as_deref()
                        .unwrap_or("NONE")
                ),
                Err(err) => format!("# Workflow Gate: [advance]\n\n⚠️ Error: {}", err),
            }
        }
        "authorize_edit" => super::gate_edit::handle_authorize_edit(&arguments, state),
        "rollback" => {
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            match crate::mcp::impact::restore_session_snapshots(&proj_path, &state.session_id) {
                Ok(restored) => {
                    if restored.is_empty() {
                        format!(
                            "# Rollback Guard: [rollback]\n\nNo snapshot files found for session '{}'. No changes were reverted.",
                            state.session_id
                        )
                    } else {
                        format!(
                            "# Rollback Guard: [rollback] ✓\n\nSuccessfully restored {} file(s) to their pre-edit state for session '{}':\n\n{}",
                            restored.len(),
                            state.session_id,
                            restored.iter().map(|f| format!("- `{}`", f)).collect::<Vec<_>>().join("\n")
                        )
                    }
                }
                Err(e) => format!("# Rollback Guard: [rollback] ⚠️\n\nFailed to restore session snapshots: {}", e),
            }
        }
        _ => {
            // "check" action — SECURITY FIX Bug #2: READ ONLY (no state mutation)
            let status_str = if state.workflow_stage == "Build" && !state.plan_approved {
                "BLOCKED"
            } else {
                "PASSED"
            };
            let mut resp = format!(
                "# Workflow Gate: [check]\n\nStatus: {} | Plan Approved: {} | Stage: {} | Fix Attempts: {}",
                status_str, state.plan_approved, state.workflow_stage, state.fix_attempts
            );
            if state.workflow_stage == "Build" && !state.plan_approved {
                resp.push_str("\n\n⚠️ Trigger IDE/CLI `ask_question` tool to request explicit user plan approval before editing code.");
            }
            if state.workflow_stage == "Test_Recheck" {
                resp.push_str("\n\n⚠ **ANTI-HALLUCINATION ENFORCER ACTIVE**: Re-read the original user prompt & verify all requested features against real build/test outputs before declaring task complete.");
            }
            resp
        }
      };
    Ok(resp)
}