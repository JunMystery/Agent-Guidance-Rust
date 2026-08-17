use serde_json::Value;

use crate::mcp::state::ServerState;
use super::helpers::{detect_project_path, resolve_architecture_pattern};

pub(crate) fn handle_authorize_edit(
    arguments: &Value,
    state: &mut ServerState,
) -> String {
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);
    state.update_project_path(&proj_path);
    let rel_path = arguments
        .get("relative_path")
        .and_then(|r| r.as_str())
        .unwrap_or("");
    let justification = arguments
        .get("justification")
        .and_then(|j| j.as_str())
        .unwrap_or("");
    let raw_arch = arguments
        .get("architecture_pattern")
        .and_then(|a| a.as_str())
        .unwrap_or("Auto");
    let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path, state);

    // Zero-Turn Predictive Transition: if plan approved and in Plan stage, auto-advance to Build
    if state.plan_approved && state.workflow_stage == "Plan" {
        let _ = state.set_stage("Build");
    }

    // Perform Code Graph Diff Impact Guard analysis
    let impact = crate::mcp::impact::assess_file_risk(&proj_path, rel_path);
    let declared_risk = arguments
        .get("risk_level")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| match impact.risk_level {
            crate::mcp::impact::RiskLevel::High => "HIGH".to_string(),
            crate::mcp::impact::RiskLevel::Medium => "MEDIUM".to_string(),
            crate::mcp::impact::RiskLevel::Low => "LOW".to_string(),
        });
    state.last_risk_level = Some(declared_risk.clone());

    if !matches!(
        arch_pattern.as_str(),
        "Clean_Architecture"
            | "Layered_Architecture"
            | "Package_By_Feature"
            | "Orchestrator"
            | "CLI_Pipeline"
            | "Flat_Library"
    ) {
        format!(
            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (ORCHESTRATION MANDATE VIOLATION)\n- Project Path: {}\n- Declared Architecture: '{}'\n\n⚠️ Error: ARCHITECTURE_GATE_BLOCKED: Trigger IDE/CLI `ask_question` tool to let user choose a valid `architecture_pattern` ('Clean_Architecture', 'Layered_Architecture', 'Package_By_Feature', 'Orchestrator', 'CLI_Pipeline', 'Flat_Library', or 'Auto'), then re-invoke `workflow_gate(action=\"authorize_edit\", ...) `.",
            proj_path.display(),
            if arch_pattern.is_empty() {
                "NONE"
            } else {
                &arch_pattern
            }
        )
    } else if impact.risk_level == crate::mcp::impact::RiskLevel::High && (justification.trim().len() < 10 || justification == "No justification provided") {
        // Strict Gate for Critical Hub: mandatory explanation
        format!(
            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (CRITICAL HUB IMPACT GUARD)\n- Target File: `{}`\n- Incoming Dependencies: {}\n- Impacted Modules: {}\n\n⚠️ **Error: HIGH_RISK_JUSTIFICATION_REQUIRED**: This file is a Critical Hub (referenced by >8 modules). You MUST provide a specific `justification` parameter explaining how your changes avoid breaking downstream modules, along with your planned test verification.",
            rel_path,
            impact.dependent_count,
            if impact.dependent_files.is_empty() { "—".to_string() } else { impact.dependent_files.join(", ") }
        )
    } else if declared_risk == "HIGH" && !state.plan_approved {
        format!(
            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (HIGH RISK)\n- Project Path: {}\n- Declared Risk: HIGH\n- Justification: {}\n\n⚠️ Error: HIGH RISK edits require explicit user approval. Present plan and trigger IDE/CLI `ask_question` tool (or invoke `workflow_gate(action=\"set_stage\", target_stage=\"Plan\")`) to confirm approval.",
            proj_path.display(),
            if justification.is_empty() { "No justification provided" } else { justification }
        )
    } else if state.workflow_stage == "Build" && state.plan_approved {
        state.edit_authorized = true;
        state.active_architecture_pattern = Some(arch_pattern.clone());

        // Automatically record modified file and take pre-edit snapshot for rollback guard
        if !rel_path.is_empty() {
            state.record_modified_file(rel_path);
            let _ = crate::mcp::impact::create_file_snapshot(&proj_path, rel_path, &state.session_id);
        }
        let _ = state.auto_checkpoint(&proj_path);

        let mut resp = format!(
            "# Edit Approval Gate Authorization\n\n- Status: PASSED\n- Project Path: {}\n- Target File: {}\n- Assessed Risk Level: {:?} (Dependencies: {})\n- Architecture Pattern: {}\n- Justification: {}\n- Active Stage: {}\n- Plan Approved: true\n\n✓ File edits are fully authorized under {} Architecture.",
            proj_path.display(),
            if rel_path.is_empty() { "—" } else { rel_path },
            impact.risk_level,
            impact.dependent_count,
            arch_pattern,
            if justification.is_empty() { "Standard development task" } else { justification },
            state.workflow_stage,
            arch_pattern
        );

        if let Some(warn) = impact.warning {
            resp.push_str(&format!("\n\n⚠️ **Impact Guard**: {}", warn));
        }

        resp
    } else {
        format!(
            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED\n- Project Path: {}\n- Active Stage: {}\n- Plan Approved: {}\n\n⚠️ Error: WORKFLOW_STAGE_BLOCKED: Edits require Build stage and plan_approved=true. Trigger IDE/CLI `ask_question` tool to request user approval on the plan, then invoke `workflow_gate(action=\"set_stage\", target_stage=\"Build\")`.",
            proj_path.display(),
            state.workflow_stage,
            state.plan_approved
        )
    }
}
