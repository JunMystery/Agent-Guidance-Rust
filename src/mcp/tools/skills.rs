use serde_json::{Value, json};
use std::path::Path;

use crate::catalog::language_detector::detect_language_fast;
use crate::catalog::store::get_embedded_skill;
use crate::mcp::state::ServerState;
use crate::optimizer::compressor::{compress_markdown, estimate_tokens};
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

    let resp = if requested_skills.is_empty() {
        let text = "# Skill Selection\n\nNo skills selected. Proceeding directly to task execution.\n\n-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. Otherwise, answer directly or proceed to task planning.".to_string();
        let tokens = estimate_tokens(&text, false) as u64;
        state.record_call(tokens, tokens);
        text
    } else {
        let mut loaded_sections = Vec::new();
        let mut not_found = Vec::new();
        let mut raw_token_acc: usize = 0;

        for name in &requested_skills {
            if let Some((_, rel_path, _)) = proposals.iter().find(|(n, _, _)| n == name) {
                crate::mcp::db::log_skill_load(name);

                let raw_content = if let Some(c) = get_embedded_skill(name) {
                    Some(c)
                } else if let Some(c) = get_embedded_skill(rel_path) {
                    Some(c)
                } else if let Ok(full_path) = validate_path(&proj_path, rel_path) {
                    std::fs::read_to_string(&full_path).ok()
                } else if let Ok(full_path) = validate_path(&proj_path, name) {
                    std::fs::read_to_string(&full_path).ok()
                } else {
                    None
                };

                if let Some(content) = raw_content {
                    raw_token_acc += estimate_tokens(&content, false);
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
                raw_token_acc += estimate_tokens(&c, false);
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
                    raw_token_acc += estimate_tokens(&c, false);
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

        let profile = detect_language_fast(&proj_path, task_arg);
        let safety_rules = crate::catalog::slicing::get_language_safety_rules(&profile);

        let mut resp = format!(
            "# Skill Selection Confirmed ({})\n\nLoaded Skills Content:\n\n{}\n\n## 🛡️ Language Safety Rules\n{}",
            loaded_sections.len(),
            loaded_sections.join("\n\n---\n\n"),
            safety_rules
        );

        if !not_found.is_empty() {
            resp.push_str(&format!(
                "\n\n⚠️ Skills not found:\n{}",
                not_found.iter().map(|n| format!("- {}", n)).collect::<Vec<_>>().join("\n")
            ));
        }

        resp.push_str("\n\n-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. Otherwise, answer directly or proceed to task planning.");

        let opt_tokens = estimate_tokens(&resp, false);
        let orig_tokens = raw_token_acc.max(opt_tokens);
        state.record_call(orig_tokens as u64, opt_tokens as u64);

        resp
    };
    Ok(resp)
}