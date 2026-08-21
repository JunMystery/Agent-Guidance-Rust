use serde_json::{Value, json};
use std::path::Path;

use crate::catalog::store::{SkillSource, get_embedded_skill, list_embedded_skills, load_all_skills};
use crate::context::cache::project_snapshot;
use crate::mcp::state::ServerState;
use crate::ml::embeddings::hybrid_vector_search;
use crate::ml::llm_selector::LLMSelector;
use crate::optimizer::compressor::compress_markdown;
use super::{detect_project_path, ensure_not_cancelled, validate_path};

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let requested_skills: Vec<String> = arguments
        .get("skills")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let user_confirmed = arguments
        .get("user_confirmed")
        .or_else(|| arguments.get("confirmed"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let user_msg = arguments.get("user_message").and_then(|u| u.as_str());
    if let Some(msg) = user_msg {
        state.process_user_message(msg);
    }

    let proposals = std::mem::take(&mut state.pending_skill_proposals);

    // Hardened Guard: If skills are requested while proposals were pending, require explicit user confirmation
    if !requested_skills.is_empty() && !proposals.is_empty() && !user_confirmed && user_msg.is_none() {
        // Restore pending proposals so user choice can still be applied
        state.pending_skill_proposals = proposals;
        return Ok("# Skill Selection Gate: [select_skills]\n\nStatus: BLOCKED | Error: USER_CONFIRMATION_REQUIRED: You must NEVER auto-select skills on behalf of the user. Trigger the IDE/CLI `ask_question` tool so the user selects which skills to inject. Once the user submits their choice, re-invoke `select_skills(skills=[...], user_confirmed=true)`.".to_string());
    }

    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);

    let task_arg = arguments
        .get("task")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let core_rules = crate::catalog::rules::format_skill_load_rules();

    let resp = if requested_skills.is_empty() {
        state.record_call(100, 50);
        format!(
            "# Skill Selection\n\nNo skills selected. Proceeding without loading skills.\n\n{}\n\n-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. Otherwise, answer directly or proceed to task planning.",
            core_rules
        )
    } else {
        let mut loaded_sections = Vec::new();
        let mut not_found = Vec::new();

        for name in &requested_skills {
            if let Some((_, rel_path, _)) = proposals.iter().find(|(n, _, _)| n == name) {
                crate::mcp::db::log_skill_load(name);

                let raw_content = if let Some(c) = get_embedded_skill(name) {
                    Some(c)
                } else if let Some(c) = get_embedded_skill(rel_path) {
                    Some(c)
                } else if let Ok(c) = std::fs::read_to_string(name) {
                    Some(c)
                } else if let Ok(c) = std::fs::read_to_string(rel_path) {
                    Some(c)
                } else if let Ok(full_path) = validate_path(&proj_path, rel_path) {
                    std::fs::read_to_string(&full_path).ok()
                } else {
                    None
                };

                if let Some(content) = raw_content {
                    let processed = if !task_arg.is_empty() {
                        crate::catalog::slicing::slice_skill_markdown(&content, task_arg, 3)
                    } else {
                        compress_markdown(&content)
                    };
                    loaded_sections.push(format!(
                        "### Skill: {}\n```markdown\n{}\n```",
                        name, processed
                    ));
                } else {
                    loaded_sections.push(format!(
                        "### Skill: {} (Loaded & Logged)\n*Content empty or unavailable*",
                        name
                    ));
                }
            } else if let Some(c) = get_embedded_skill(name) {
                crate::mcp::db::log_skill_load(name);
                let processed = if !task_arg.is_empty() {
                    crate::catalog::slicing::slice_skill_markdown(&c, task_arg, 3)
                } else {
                    compress_markdown(&c)
                };
                loaded_sections.push(format!(
                    "### Skill: {} [Embedded Catalog]\n```markdown\n{}\n```",
                    name, processed
                ));
            } else if let Ok(full_path) = validate_path(&proj_path, name) {
                if let Ok(c) = std::fs::read_to_string(&full_path) {
                    crate::mcp::db::log_skill_load(name);
                    let processed = if !task_arg.is_empty() {
                        crate::catalog::slicing::slice_skill_markdown(&c, task_arg, 3)
                    } else {
                        compress_markdown(&c)
                    };
                    loaded_sections.push(format!(
                        "### Skill: {} [Local Workspace]\n```markdown\n{}\n```",
                        name, processed
                    ));
                } else {
                    not_found.push(name.clone());
                }
            } else {
                not_found.push(name.clone());
            }
        }

        let snapshot = project_snapshot(&proj_path);
        let profile = crate::catalog::language_detector::detect_language_profile(
            snapshot.files.as_ref(),
            task_arg,
        );
        let safety_rules = crate::catalog::slicing::get_language_safety_rules(&profile);

        state.record_call(1500, 500);
        let mut resp = format!(
            "# Skill Selection Confirmed ({})\n\nLoaded Skills Content:\n\n{}\n\n{}\n\n## 🛡️ Language Safety Rules\n{}",
            loaded_sections.len(),
            loaded_sections.join("\n\n---\n\n"),
            core_rules,
            safety_rules
        );

        if !not_found.is_empty() {
            resp.push_str(&format!(
                "\n\n⚠️ Skills not found:\n{}",
                not_found.iter().map(|n| format!("- {}", n)).collect::<Vec<_>>().join("\n")
            ));
        }

        resp.push_str("\n\n-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. Otherwise, answer directly or proceed to task planning.");
        resp
      };
    Ok(resp)
}