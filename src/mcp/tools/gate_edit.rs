use serde_json::Value;
use std::path::Path;

use crate::mcp::state::ServerState;
use super::gate_edit_batch::handle_batch_authorize;
use super::gate_edit_single::evaluate_single_file_auth;
use super::helpers::{detect_project_path, resolve_architecture_pattern};

pub(crate) fn extract_target_paths(arguments: &Value) -> Vec<String> {
    if let Some(arr) = arguments
        .get("relative_paths")
        .or_else(|| arguments.get("files"))
        .and_then(|v| v.as_array())
    {
        return arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Some(s) = arguments
        .get("relative_path")
        .or_else(|| arguments.get("file_path"))
        .or_else(|| arguments.get("path"))
        .and_then(|v| v.as_str())
    {
        let trimmed = s.trim();
        if trimmed.contains(',') {
            return trimmed
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
        }
        if !trimmed.is_empty() {
            return vec![trimmed.to_string()];
        }
    }
    Vec::new()
}

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

    let paths = extract_target_paths(arguments);
    let justification = arguments
        .get("justification")
        .and_then(|j| j.as_str())
        .unwrap_or("");
    let raw_arch = arguments
        .get("architecture_pattern")
        .and_then(|a| a.as_str())
        .unwrap_or("Auto");
    let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path, state);

    if paths.is_empty() {
        return format!(
            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (RELATIVE_PATH_REQUIRED)\n- Project Path: {}\n- Declared Architecture: '{}'\n\n**Error: RELATIVE_PATH_REQUIRED**: `workflow_gate(action=\"authorize_edit\")` requires target `relative_path` or `relative_paths` (for batch authorization).\n\nBlanket or global edit authorizations without a target file are prohibited to enforce:\n1. **< 300 LOC Cap & Decomposition**: Preventing monolithic files from being created or expanded.\n2. **Code Graph Diff Impact Guard**: Evaluating module blast radius and incoming dependencies.\n3. **Pre-Edit Rollback Snapshot**: Creating safe checkpoints for rollback protection.\n4. **Scanned Architecture Alignment**: Guiding modular sub-module placement from line 1.\n\n**Action**: Call `workflow_gate(action=\"authorize_edit\", project_path=\"...\", relative_paths=[\"path1\", \"path2\"])` to authorize all planned files in a single turn.",
            proj_path.display(),
            if arch_pattern.is_empty() { "Auto" } else { &arch_pattern }
        );
    }

    // Zero-Turn Predictive Transition: if plan approved and in Plan stage, auto-advance to Build
    if state.plan_approved && state.workflow_stage == "Plan" {
        let _ = state.set_stage("Build");
    }

    if !matches!(
        arch_pattern.as_str(),
        "Clean_Architecture"
            | "Layered_Architecture"
            | "Package_By_Feature"
            | "Orchestrator"
            | "CLI_Pipeline"
            | "Flat_Library"
    ) {
        return format!(
            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (ORCHESTRATION MANDATE VIOLATION)\n- Project Path: {}\n- Declared Architecture: '{}'\n\nError: ARCHITECTURE_GATE_BLOCKED: Trigger IDE/CLI `ask_question` tool to let user choose a valid `architecture_pattern` ('Clean_Architecture', 'Layered_Architecture', 'Package_By_Feature', 'Orchestrator', 'CLI_Pipeline', 'Flat_Library', or 'Auto'), then re-invoke `workflow_gate(action=\"authorize_edit\", ...) `.",
            proj_path.display(),
            if arch_pattern.is_empty() { "NONE" } else { &arch_pattern }
        );
    }

    if state.workflow_stage != "Build" || !state.plan_approved {
        return format!(
            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED\n- Project Path: {}\n- Active Stage: {}\n- Plan Approved: {}\n\nError: WORKFLOW_STAGE_BLOCKED: Edits require Build stage and plan_approved=true. If user approved via GUI/chat, invoke `workflow_gate(action=\"set_stage\", target_stage=\"Build\", user_confirmed=true)`. Only trigger `ask_question` if interactive user approval is required.",
            proj_path.display(),
            state.workflow_stage,
            state.plan_approved
        );
    }

    let is_batch = arguments.get("relative_paths").is_some()
        || arguments.get("files").is_some()
        || paths.len() > 1;

    if is_batch {
        return handle_batch_authorize(&proj_path, &paths, justification, &arch_pattern, state);
    }

    // Single file authorization flow
    let rel_path = &paths[0];
    let outcome = evaluate_single_file_auth(
        &proj_path,
        rel_path,
        justification,
        &arch_pattern,
        state,
    );

    if !outcome.is_passed {
        return outcome.error_msg.unwrap_or_else(|| {
            format!(
                "# Edit Approval Gate Authorization\n\n- Status: BLOCKED\n- File: `{}`\n\nValidation failed.",
                rel_path
            )
        });
    }

    state.edit_authorized = true;
    state.active_architecture_pattern = Some(arch_pattern.clone());
    let _ = state.auto_checkpoint(&proj_path);

    let is_exempt = is_exempt_from_loc_limit(rel_path);
    let mut resp = format!(
        "# Edit Approval Gate Authorization\n\n- Status: PASSED{}\n- Project Path: {}\n- Target File: `{}`{}\n- Assessed Risk Level: {:?} (Dependencies: {})\n- Architecture Pattern: {}\n- Justification: {}\n- Active Stage: {}\n- Plan Approved: true\n\nFile edits are fully authorized under {} Architecture.",
        if outcome.target_loc >= 300 && !is_exempt { " (DECOMPOSITION / REFACTOR MODE)" } else { "" },
        proj_path.display(),
        rel_path,
        if outcome.is_new { " [NEW FILE]" } else { "" },
        outcome.risk_level,
        outcome.dependent_count,
        arch_pattern,
        if justification.is_empty() { "Standard development task" } else { justification },
        state.workflow_stage,
        arch_pattern
    );

    if outcome.is_new && !is_exempt {
        resp.push_str("\n\n[LOC Limit: strictly < 300 LOC (target < 150 LOC)]");
    } else if outcome.target_loc >= 300 && !is_exempt {
        resp.push_str(&format!(
            "\n\n**300 LOC Modular Refactoring Mandate**: {} lines (>= 300 LOC limit). Refactoring/decomposition only.",
            outcome.target_loc
        ));
    }

    if let Some(warn) = outcome.warning {
        resp.push_str(&format!("\n\n**Impact Guard**: {}", warn));
    }

    if let Some(tip) = outcome.co_change_tip {
        resp.push_str(&format!(
            "\n\n> [!TIP] **Co-Change Coupling Alert**: {}",
            tip
        ));
    }

    resp
}

pub fn is_exempt_from_loc_limit(rel_path: &str) -> bool {
    let path = Path::new(rel_path);
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        matches!(
            ext_lower.as_str(),
            "md" | "markdown" | "mdown" | "mdx" | "txt" | "rst" | "adoc" | "doc" | "docx" | "pdf"
            | "json" | "json5" | "jsonl" | "csv" | "tsv" | "yaml" | "yml" | "xml" | "toml" | "lock"
            | "sql" | "env" | "ini" | "cfg" | "properties" | "parquet" | "dataset" | "arrow" | "proto" | "graphql" | "gql"
            | "html" | "htm" | "css" | "scss" | "sass" | "less" | "svg" | "png" | "jpg" | "jpeg" | "gif" | "ico" | "webp"
            | "log" | "diff" | "patch"
        )
    } else {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let name_upper = name.to_uppercase();
        matches!(
            name_upper.as_str(),
            "LICENSE" | "LICENCE" | "README" | "CHANGELOG" | "AUTHORS" | "CONTRIBUTING" | ".GITIGNORE" | ".ENV"
        )
    }
}
