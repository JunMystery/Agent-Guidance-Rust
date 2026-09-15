use std::path::Path;

use crate::mcp::impact::{assess_file_risk, create_file_snapshot, RiskLevel};
use crate::mcp::state::ServerState;
use super::gate_edit::is_exempt_from_loc_limit;
use super::gate_edit_modularity::validate_new_file_modularity;
use super::gate_stage::count_file_lines;
use super::helpers::validate_path;

#[derive(Debug, Clone)]
pub(crate) struct SingleFileAuthOutcome {
    pub rel_path: String,
    pub is_passed: bool,
    pub is_new: bool,
    pub target_loc: usize,
    pub risk_level: RiskLevel,
    pub dependent_count: usize,
    pub error_msg: Option<String>,
    pub warning: Option<String>,
    pub co_change_tip: Option<String>,
}

pub(crate) fn is_refactor_intent(justification: &str) -> bool {
    let j_lower = justification.to_lowercase();
    j_lower.contains("refactor")
        || j_lower.contains("decompose")
        || j_lower.contains("decomposition")
        || j_lower.contains("extract")
        || j_lower.contains("split")
        || j_lower.contains("thin dispatcher")
        || j_lower.contains("reduce loc")
        || j_lower.contains("tách file")
        || j_lower.contains("rút gọn")
}

pub(crate) fn evaluate_single_file_auth(
    proj_path: &Path,
    rel_path: &str,
    justification: &str,
    arch_pattern: &str,
    state: &mut ServerState,
) -> SingleFileAuthOutcome {
    let mut outcome = SingleFileAuthOutcome {
        rel_path: rel_path.to_string(),
        is_passed: false,
        is_new: false,
        target_loc: 0,
        risk_level: RiskLevel::Low,
        dependent_count: 0,
        error_msg: None,
        warning: None,
        co_change_tip: None,
    };

    if let Err(err_msg) = validate_path(proj_path, rel_path) {
        outcome.error_msg = Some(format!("PATH_TRAVERSAL_PROHIBITED: {}", err_msg));
        return outcome;
    }

    let impact = assess_file_risk(proj_path, rel_path);
    outcome.risk_level = impact.risk_level.clone();
    outcome.dependent_count = impact.dependent_count;
    outcome.warning = impact.warning;

    let full_target_path = proj_path.join(rel_path);
    outcome.is_new = !full_target_path.exists();
    outcome.target_loc = if full_target_path.exists() && full_target_path.is_file() {
        count_file_lines(&full_target_path)
    } else {
        0
    };

    let is_exempt = is_exempt_from_loc_limit(rel_path);
    let refactor_intent = is_refactor_intent(justification);

    if outcome.is_new && !is_exempt {
        if let Err(err_msg) = validate_new_file_modularity(rel_path, justification) {
            outcome.error_msg = Some(err_msg);
            return outcome;
        }
    }

    if outcome.target_loc >= 300 && !is_exempt && !refactor_intent {
        let msg = crate::catalog::blueprint::format_decomposition_guidance(
            rel_path,
            outcome.target_loc,
            arch_pattern,
        );
        outcome.error_msg = Some(msg);
        return outcome;
    }

    if impact.risk_level == RiskLevel::High
        && (justification.trim().len() < 10 || justification == "No justification provided")
    {
        outcome.error_msg = Some(format!(
            "HIGH_RISK_JUSTIFICATION_REQUIRED: Critical Hub (>8 callers). Justification explaining test mitigation required for `{}`.",
            rel_path
        ));
        return outcome;
    }

    // Success path: Record modified file, take snapshot, and check coupling
    state.record_modified_file(rel_path);
    let _ = create_file_snapshot(proj_path, rel_path, &state.session_id);
    outcome.is_passed = true;

    if let Ok(db) = crate::context::db::CodeGraphDb::open_for_project(proj_path) {
        if let Ok(coupled) = crate::context::co_change::predict_coupled_files(
            &db.conn,
            &[rel_path.to_string()],
            1,
            0.6,
        ) {
            if let Some(top) = coupled.first() {
                outcome.co_change_tip = Some(format!(
                    "Historically `{}` is modified together with `{}` ({} co-changes, {:.0}% confidence).",
                    top.coupled_with, top.file, top.co_change_count, top.confidence * 100.0
                ));
            }
        }
    }

    outcome
}
