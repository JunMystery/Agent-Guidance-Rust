use serde_json::Value;

use crate::mcp::state::ServerState;
use super::gate_approval::{
    handle_approve_plan, handle_pass_verification, handle_rollback, handle_set_architecture,
};
use super::gate_edit::handle_authorize_edit;
use super::gate_stage::{handle_advance, handle_check, handle_set_stage, handle_status};
use super::helpers::{detect_project_path, ensure_not_cancelled};

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
        .or_else(|| arguments.get("plan_approved"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let user_msg = arguments.get("user_message").and_then(|u| u.as_str());

    if user_confirmed {
        state.approve_plan();
    }
    if let Some(msg) = user_msg {
        state.process_user_message(msg);
    }

    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);

    let resp = match action {
        "approve" | "approve_plan" => {
            handle_approve_plan(state, user_confirmed, user_msg, &proj_path)
        }
        "pass_verification" => handle_pass_verification(state, &proj_path),
        "set_stage" => handle_set_stage(state, stage_target, &proj_path),
        "status" => handle_status(state),
        "set_architecture" => handle_set_architecture(&arguments, state, &proj_path),
        "advance" | "advance_stage" => handle_advance(&arguments, state, &proj_path),
        "authorize_edit" => handle_authorize_edit(&arguments, state),
        "rollback" => handle_rollback(state, &proj_path),
        _ => handle_check(state),
    };
    Ok(resp)
}