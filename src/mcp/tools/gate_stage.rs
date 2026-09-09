use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::mcp::state::ServerState;
use super::gate_edit::is_exempt_from_loc_limit;
use super::helpers::resolve_architecture_pattern;

pub(crate) fn count_file_lines(path: &Path) -> usize {
    if let Ok(file) = std::fs::File::open(path) {
        BufReader::new(file).lines().count()
    } else {
        0
    }
}

pub(crate) fn handle_set_stage(
    state: &mut ServerState,
    stage_target: Option<&str>,
    proj_path: &Path,
) -> String {
    if let Some(target) = stage_target {
        match state.set_stage(target) {
            Ok(new_stage) => {
                let _ = state.auto_checkpoint(proj_path);
                format!(
                    "# Workflow Gate: [set_stage]\n\nStatus: PASSED | Stage Changed To: {} | Plan Approved: {} | Fix Attempts: {}",
                    new_stage, state.plan_approved, state.fix_attempts
                )
            }
            Err(err_msg) => {
                let _ = state.auto_checkpoint(proj_path);
                format!(
                    "# Workflow Gate: [set_stage]\n\nStatus: BLOCKED | Error: {}. If user approved via GUI or chat, pass user_confirmed=true (or trigger ask_question only if interactive clarification needed).",
                    err_msg
                )
            }
        }
    } else {
        "# Workflow Gate: [set_stage]\n\nStatus: BLOCKED | Error: target_stage argument is required for set_stage action.".to_string()
    }
}

pub(crate) fn handle_advance(
    arguments: &Value,
    state: &mut ServerState,
    proj_path: &Path,
) -> String {
    let target_stage = arguments
        .get("target_stage")
        .and_then(|t| t.as_str())
        .unwrap_or("Build");

    let user_confirmed = arguments
        .get("user_confirmed")
        .or_else(|| arguments.get("confirmed"))
        .or_else(|| arguments.get("plan_approved"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if user_confirmed {
        state.approve_plan();
    }

    // 300 LOC Post-Edit Hard Trap: Block advancing if any modified/created code file >= 300 LOC
    if target_stage == "Proposal" || target_stage == "Review" {
        for file_rel in &state.modified_files {
            if is_exempt_from_loc_limit(file_rel) {
                continue;
            }
            let full = proj_path.join(file_rel);
            if full.exists() && full.is_file() {
                let loc = count_file_lines(&full);
                if loc >= 300 {
                    return format!(
                        "# Workflow Gate: BLOCKED (300_LOC_CAP_EXCEEDED)\n\n- Target File: `{}` (Current Length: **{} lines** / limit: 300 lines)\n\n**Error: 300_LOC_LIMIT_EXCEEDED**: Newly created or modified code file '{}' has {} lines, which breaches the 300 LOC limit. You cannot advance workflow stages until this file is decomposed into modular sub-files (< 150 LOC each).\n\n**Action Required**: Decompose `{}` into focused sub-modules using `workflow_gate(action=\"authorize_edit\", justification=\"Refactor/Decompose: ...\")`.",
                        file_rel, loc, file_rel, loc, file_rel
                    );
                }
            }
        }
    }

    let stage_res = state.set_stage(target_stage);

    let raw_arch = arguments
        .get("architecture_pattern")
        .and_then(|a| a.as_str())
        .unwrap_or("Auto");
    let risk_level = arguments
        .get("risk_level")
        .and_then(|r| r.as_str())
        .unwrap_or("LOW");
    state.last_risk_level = Some(risk_level.to_string());

    let arch_pattern = resolve_architecture_pattern(raw_arch, proj_path, state);

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
        let _ = state.auto_checkpoint(proj_path);
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
        Err(err) => format!("# Workflow Gate: [advance]\n\nError: {}", err),
    }
}

pub(crate) fn handle_check(state: &ServerState) -> String {
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
        resp.push_str("\n\nPlan approval required before editing code. If user approved via GUI/chat, invoke workflow_gate with user_confirmed=true (or ask_question if interactive approval needed).");
    }
    if state.workflow_stage == "Test_Recheck" {
        resp.push_str("\n\n**ANTI-HALLUCINATION ENFORCER ACTIVE**: Re-read the original user prompt & verify all requested features against real build/test outputs before declaring task complete.");
    }
    resp
}

pub(crate) fn handle_status(state: &ServerState) -> String {
    let edit_allowed = state.workflow_stage == "Build" && state.plan_approved;
    format!(
        "# Workflow Stage Status\n\n- Active Stage: {}\n- Plan Approved: {}\n- Fix Attempts: {}/3\n- Edit Authorized: {}",
        state.workflow_stage, state.plan_approved, state.fix_attempts, edit_allowed
    )
}
