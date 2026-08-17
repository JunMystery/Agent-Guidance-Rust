use serde_json::{Value, json};
use std::path::Path;

use crate::catalog::language_detector;
use crate::catalog::store::{list_embedded_skills, load_all_skills};
use crate::context::cache::project_snapshot;
use crate::context::scanner::scan_project;
use crate::mcp::state::ServerState;
use crate::ml::embeddings::hybrid_vector_search;
use crate::ml::llm_selector::LLMSelector;
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

    let focus = arguments
        .get("focus")
        .and_then(|f| f.as_str())
        .map(|f| f.trim())
        .filter(|f| !f.is_empty() && *f != "general");
    let search_query = if let Some(f) = focus {
        format!("{} {}", task, f)
    } else {
        task.to_string()
    };

    let snapshot = project_snapshot(&proj_path);
    ensure_not_cancelled(state)?;
    let file_count = snapshot.files.len();
    let profile = crate::catalog::language_detector::detect_language_profile(
        snapshot.files.as_ref(),
        &search_query,
    );

    let stage1_results = hybrid_vector_search(&search_query, snapshot.skills.as_ref(), 16);
    ensure_not_cancelled(state)?;
    let selector = LLMSelector::new();
    let final_results = selector.rerank(&search_query, stage1_results, &profile, 16);
    ensure_not_cancelled(state)?;

    let mut seen_names = std::collections::HashSet::new();
    let mut deduped_results = Vec::new();
    for (score, item) in final_results {
        if seen_names.insert(item.name.clone()) {
            deduped_results.push((score, item));
            if deduped_results.len() >= 8 {
                break;
            }
        }
    }

    state.pending_skill_proposals = deduped_results
        .iter()
        .map(|(score, item)| (item.name.clone(), item.relative_path.clone(), *score))
        .collect();

    let rec_skills: Vec<String> = deduped_results
        .iter()
        .map(|(score, item)| format!("- {} (Score: {:.2})", item.name, score))
        .collect();

    let execution_seq = "- Step 1: Context & Specification\n- Step 2: Architecture & Implementation Plan\n- Step 3: Code Implementation (Build stage)\n- Step 4: Verification & Testing\n- Step 5: Post-Code Review & Documentation";
    let tree_preview: Vec<String> = snapshot
        .files
        .iter()
        .take(15)
        .map(|f| format!("- {} ({})", f.path, f.file_type))
        .collect();

    let next_step_prompt = if rec_skills.is_empty() {
        "-> CODEBASE EXPLORATION: Use `project_context(operation=\"search\", query=\"...\")` to locate code and `project_context(operation=\"read\", relative_path=\"...\")` to inspect functions. Do NOT use raw grep_search, list_dir, or view_file."
    } else {
        "-> SKILL_PROPOSAL: Trigger IDE/CLI `ask_question` tool to present recommended skills interactively to user, then call `select_skills(skills=[...])` with chosen skills (or `select_skills(skills=[])` if skipped).\n-> MANDATORY NEXT STEP: Use `project_context(operation=\"search\", query=\"...\")` to find files and `project_context(operation=\"read\", relative_path=\"...\")` to inspect code. Avoid raw filesystem tools."
    };

    let detected_arch = detect_project_architecture(&proj_path);
    state.active_architecture_pattern = Some(detected_arch.clone());

    let core_rules_checklist = "## Mandatory Agent Execution Mandates (9 Core Rules)\n1. **Context First**: Always run `task_pipeline` or `project_context` before reading files or modifying code.\n2. **Fast Edit Authorization**: Must call `workflow_gate(action=\"authorize_edit\")` before editing.\n3. **Token Budget**: Max 300 lines per read, use symbol extraction over full-file dumps.\n4. **No Direct FS**: Prioritize MCP tools over raw filesystem access.\n5. **Ground & Plan**: Verify codebase facts via search before proposing changes.\n6. **Upfront Architecture & 300 LOC Cap**: Enforce 300 LOC limit from line 1. Split entry dispatchers from sub-module handlers upfront.\n7. **Intent Gate**: Classify request type before acting.\n8. **Delegation First**: Decompose and delegate multi-step tasks when applicable.\n9. **Phase Progression**: Complete Context -> Plan -> Build -> Test -> Review sequence.";

    let dynamic_blueprint = crate::catalog::blueprint::generate_dynamic_blueprint(&proj_path, task, &detected_arch);
    let skill_recipe = crate::catalog::blueprint::generate_skill_recipe(&deduped_results, task);
    let relevant_learnings = crate::mcp::learnings::get_semantic_relevant_learnings(&proj_path, task, 3, 0.82);

    let learnings_section = if relevant_learnings.is_empty() {
        String::new()
    } else {
        format!("\n\n## 💡 Project Memorized Learnings\n{}", relevant_learnings.join("\n"))
    };

    state.record_call(1500, 450);
    Ok(format!(
        "# Task Pipeline Activated\n\nTask: {}\nActive Phase: {}\nProject: {}\n\n## Recommendations\n{}{}\n\n## 🍳 Task-Specific Skill Recipe\n{}\n\n## 📐 Dynamic Split Blueprint\n{}\n\n## Architecture Guidance\n- Active Pattern: {}\n- Enforce: Create thin dispatcher main + sub-module files from line 1 (Upfront Architecture, 300 LOC Cap)\n\n{}\n\n## Execution Sequence\n{}\n\n## Project Tree (Scanned Files: {})\n{}\n\nPriority Gate: PASSED\nStatus: Ready for execution.\n\n{}",
        task,
        phase,
        proj_path.display(),
        if rec_skills.is_empty() {
            "No specific skill recommendations required for this task (Token budget saved)."
                .to_string()
        } else {
            rec_skills.join("\n")
        },
        learnings_section,
        skill_recipe,
        dynamic_blueprint,
        detected_arch,
        core_rules_checklist,
        execution_seq,
        file_count,
        tree_preview.join("\n"),
        next_step_prompt
    ))
}