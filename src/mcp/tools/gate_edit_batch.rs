use std::path::Path;

use crate::mcp::state::ServerState;
use super::gate_edit_single::evaluate_single_file_auth;

pub(crate) fn handle_batch_authorize(
    proj_path: &Path,
    paths: &[String],
    justification: &str,
    arch_pattern: &str,
    state: &mut ServerState,
) -> String {
    if paths.is_empty() {
        return "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (NO_FILES_SPECIFIED)\n\nError: `relative_paths` array is empty. Please provide at least one target file path.".to_string();
    }

    state.edit_authorized = true;
    state.active_architecture_pattern = Some(arch_pattern.to_string());

    let mut passed_count = 0;
    let mut blocked_count = 0;
    let mut details = Vec::new();
    let mut co_change_tips = Vec::new();

    for rel_path in paths {
        let clean_path = rel_path.trim().replace('\\', "/");
        if clean_path.is_empty() {
            continue;
        }

        let outcome = evaluate_single_file_auth(
            proj_path,
            &clean_path,
            justification,
            arch_pattern,
            state,
        );

        if outcome.is_passed {
            passed_count += 1;
            let status_tag = if outcome.is_new {
                "PASSED (NEW)"
            } else {
                "PASSED"
            };
            details.push(format!(
                "| `{}` | {} | {} LOC | {:?} | {} |",
                outcome.rel_path,
                status_tag,
                outcome.target_loc,
                outcome.risk_level,
                outcome
                    .warning
                    .unwrap_or_else(|| "Snapshot created".to_string())
            ));
            if let Some(tip) = outcome.co_change_tip {
                co_change_tips.push(tip);
            }
        } else {
            blocked_count += 1;
            details.push(format!(
                "| `{}` | **BLOCKED** | {} LOC | {:?} | {} |",
                outcome.rel_path,
                outcome.target_loc,
                outcome.risk_level,
                outcome
                    .error_msg
                    .unwrap_or_else(|| "Validation failed".to_string())
            ));
        }
    }

    let _ = state.auto_checkpoint(proj_path);

    let total = passed_count + blocked_count;
    let overall_status = if blocked_count == 0 {
        format!("PASSED ({}/{} authorized)", passed_count, total)
    } else if passed_count > 0 {
        format!(
            "PARTIAL_PASSED ({}/{} authorized, {} blocked)",
            passed_count, total, blocked_count
        )
    } else {
        format!("BLOCKED (0/{} authorized)", total)
    };

    let mut out = format!(
        "# Batch Edit Authorization Gate\n\n- Status: {}\n- Project Path: {}\n- Architecture Pattern: {}\n- Active Stage: {}\n- Plan Approved: true\n\n| File Path | Status | Length | Risk Level | Notes |\n|---|---|---|---|---|\n{}\n",
        overall_status,
        proj_path.display(),
        arch_pattern,
        state.workflow_stage,
        details.join("\n")
    );

    if !co_change_tips.is_empty() {
        out.push_str("\n### Co-Change Coupling Alerts\n");
        for tip in co_change_tips {
            out.push_str(&format!("- {}\n", tip));
        }
    }

    out
}
