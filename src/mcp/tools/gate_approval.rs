use serde_json::Value;
use std::path::Path;

use crate::mcp::state::ServerState;
use super::helpers::resolve_architecture_pattern;

pub(crate) fn handle_approve_plan(
    state: &mut ServerState,
    user_confirmed: bool,
    user_msg: Option<&str>,
    proj_path: &Path,
) -> String {
    if !user_confirmed && !state.plan_approved && user_msg.is_none() {
        return "# Workflow Gate: [approve_plan]\n\nStatus: BLOCKED | Error: USER_APPROVAL_REQUIRED: Plan approval cannot be self-granted by AI agents. If user approved via GUI/chat, re-invoke `workflow_gate(action=\"approve_plan\", user_confirmed=true)` (or trigger `ask_question` only if interactive choice is needed).".to_string();
    }

    state.approve_plan();
    let _ = state.auto_checkpoint(proj_path);
    format!(
        "# Workflow Gate: [approve_plan]\n\nStatus: PASSED | Plan Approved: true | Stage: {}",
        state.workflow_stage
    )
}

pub(crate) fn handle_pass_verification(
    state: &mut ServerState,
    proj_path: &Path,
) -> String {
    state.fix_attempts = 0;
    let _ = state.auto_checkpoint(proj_path);
    format!(
        "# Workflow Gate: [pass_verification]\n\nStatus: PASSED | Plan Approved: {} | Stage: {} | Fix Attempts: 0\n\nVerification passed and recorded in session checkpoint.",
        state.plan_approved, state.workflow_stage
    )
}

pub(crate) fn handle_set_architecture(
    arguments: &Value,
    state: &mut ServerState,
    proj_path: &Path,
) -> String {
    let raw_arch = arguments
        .get("architecture_pattern")
        .or_else(|| arguments.get("pattern"))
        .and_then(|a| a.as_str())
        .unwrap_or("Auto");
    state.update_project_path(proj_path);
    let arch_pattern = resolve_architecture_pattern(raw_arch, proj_path, state);
    state.active_architecture_pattern = Some(arch_pattern.clone());
    let _ = ServerState::save_persisted_architecture(proj_path, &arch_pattern);
    let _ = state.auto_checkpoint(proj_path);
    format!(
        "# Architecture Pattern Locked\n\n- Project: {}\n- Confirmed Architecture: {}\n- Persistence: Saved to `.agent-context/architecture.json`\n\nPattern memorized for all workflow stages and future sessions.",
        proj_path.display(),
        arch_pattern
    )
}

pub(crate) fn handle_rollback(
    state: &ServerState,
    proj_path: &Path,
) -> String {
    match crate::mcp::impact::restore_session_snapshots(proj_path, &state.session_id) {
        Ok(restored) => {
            if restored.is_empty() {
                format!(
                    "# Rollback Guard: [rollback]\n\nNo snapshot files found for session '{}'. No changes were reverted.",
                    state.session_id
                )
            } else {
                format!(
                    "# Rollback Guard: [rollback] SUCCESS\n\nSuccessfully restored {} file(s) to their pre-edit state for session '{}':\n\n{}",
                    restored.len(),
                    state.session_id,
                    restored.iter().map(|f| format!("- `{}`", f)).collect::<Vec<_>>().join("\n")
                )
            }
        }
        Err(e) => format!("# Rollback Guard: [rollback] ERROR\n\nFailed to restore session snapshots: {}", e),
    }
}
