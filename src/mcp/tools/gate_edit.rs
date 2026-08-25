use serde_json::Value;

use crate::mcp::state::ServerState;
use super::gate_edit_modularity::validate_new_file_modularity;
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
        .unwrap_or("")
        .trim();
    let justification = arguments
        .get("justification")
        .and_then(|j| j.as_str())
        .unwrap_or("");
    let raw_arch = arguments
        .get("architecture_pattern")
        .and_then(|a| a.as_str())
        .unwrap_or("Auto");
    let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path, state);

    if rel_path.is_empty() {
        return format!(
            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (RELATIVE_PATH_REQUIRED)\n- Project Path: {}\n- Declared Architecture: '{}'\n\n⚠️ **Error: RELATIVE_PATH_REQUIRED**: `workflow_gate(action=\"authorize_edit\")` is file-scoped and strictly requires the target `relative_path` (e.g. `relative_path: \"src/services/order_service.rs\"` or `relative_path: \"frontend/src/views/ProcurementView.tsx\"`).\n\nBlanket or global edit authorizations without a specific target file are prohibited to enforce:\n1. **< 300 LOC Cap & Decomposition**: Preventing monolithic files from being created or expanded.\n2. **Code Graph Diff Impact Guard**: Evaluating module blast radius and incoming dependencies.\n3. **Pre-Edit Rollback Snapshot**: Creating safe checkpoints for rollback protection.\n4. **Scanned Architecture Alignment**: Guiding modular sub-module placement from line 1.\n\n👉 **Action**: Call `workflow_gate(action=\"authorize_edit\", project_path=\"...\", relative_path=\"<path>\", risk_level=\"LOW\", justification=\"...\", architecture_pattern=\"Auto\")` for EACH individual file before modifying or creating it.",
            proj_path.display(),
            if arch_pattern.is_empty() {
                "Auto"
            } else {
                &arch_pattern
            }
        );
    }

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
    // Check existing target file LOC for 300 LOC architectural hard-block
    let full_target_path = proj_path.join(rel_path);
    let is_new_file = !full_target_path.exists();
    let target_loc = if full_target_path.exists() && full_target_path.is_file() {
        std::fs::read_to_string(&full_target_path)
            .map(|c| c.lines().count())
            .unwrap_or(0)
    } else {
        0
    };

    let is_refactoring_justification = {
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
    };

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
    } else if is_new_file && validate_new_file_modularity(rel_path, justification).is_err() {
        validate_new_file_modularity(rel_path, justification).unwrap_err()
    } else if target_loc >= 300 && !is_refactoring_justification {
        // Hard-block adding new logic to files >= 300 LOC
        crate::catalog::blueprint::format_decomposition_guidance(
            rel_path,
            target_loc,
            &arch_pattern,
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
            "# Edit Approval Gate Authorization\n\n- Status: PASSED{}\n- Project Path: {}\n- Target File: `{}`{}\n- Assessed Risk Level: {:?} (Dependencies: {})\n- Architecture Pattern: {}\n- Justification: {}\n- Active Stage: {}\n- Plan Approved: true\n\n✓ File edits are fully authorized under {} Architecture.",
            if target_loc >= 300 { " (DECOMPOSITION / REFACTOR MODE)" } else { "" },
            proj_path.display(),
            rel_path,
            if is_new_file { " [NEW FILE]" } else { "" },
            impact.risk_level,
            impact.dependent_count,
            arch_pattern,
            if justification.is_empty() { "Standard development task" } else { justification },
            state.workflow_stage,
            arch_pattern
        );

        if is_new_file {
            resp.push_str(&format!(
                "\n\n📐 **Upfront Modular Architecture Mandate for New File**:\n- **Hard Limit**: This new file MUST remain strictly < 300 LOC (target < 150 LOC for high cohesion).\n- **Decomposition Mandate**: Do NOT build monolithic files. Decompose complex logic (modals, tables, adapters, sub-services) into separate sub-modules from line 1 under `{}` Architecture.",
                arch_pattern
            ));
        } else if target_loc >= 300 {
            resp.push_str(&format!(
                "\n\n⚡ **Decomposition / Refactor Mode**: Target file has {} lines (>= 300 LOC). Edits are authorized strictly to extract logic into sub-modules and reduce file length.",
                target_loc
            ));
        }

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

