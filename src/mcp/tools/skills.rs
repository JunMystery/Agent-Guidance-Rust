use serde_json::Value;
use crate::catalog::language_detector::detect_language_fast;
use crate::catalog::store::load_all_skills;
use crate::mcp::state::ServerState;
use crate::optimizer::compressor::{compress_markdown, estimate_tokens};
use super::{detect_project_path, ensure_not_cancelled};
pub(crate) use super::skills_gate::clean_skill_identifier;
use super::skills_gate::check_confirmation_gate;
use super::skills_resolver::resolve_skill;

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let requested_skills: Vec<String> = {
        let val = arguments
            .get("skills")
            .or_else(|| arguments.get("skill"))
            .or_else(|| arguments.get("name"))
            .or_else(|| arguments.get("skill_name"))
            .or_else(|| arguments.get("identifier"));
        if let Some(arr) = val.and_then(|v| v.as_array()) {
            arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
        } else if let Some(s) = val.and_then(|v| v.as_str()) {
            vec![s.to_string()]
        } else {
            vec![]
        }
    };

    let parse_bool = |val: Option<&Value>| -> bool {
        val.and_then(|v| {
            v.as_bool().or_else(|| {
                v.as_str().map(|s| {
                    let l = s.trim().to_lowercase();
                    matches!(l.as_str(), "true" | "1" | "yes" | "confirmed" | "approved")
                })
            })
        }).unwrap_or(false)
    };

    let user_confirmed = parse_bool(arguments.get("user_confirmed").or_else(|| arguments.get("confirmed")));
    let autonomous = parse_bool(arguments.get("autonomous"));
    let user_msg = arguments.get("user_message").and_then(|u| u.as_str());
    let user_msg_approved = if let Some(msg) = user_msg {
        state.process_user_message(msg)
    } else {
        false
    };
    let is_confirmed = user_confirmed || autonomous || user_msg_approved;

    let proposals = std::mem::take(&mut state.pending_skill_proposals);

    let is_skip = |s: &str| -> bool {
        let l = s.trim().to_lowercase();
        matches!(l.as_str(), "none" | "skip" | "no skills" | "none of the above" | "n/a" | "cancel" | "no" | "[]")
    };
    let active_skills: Vec<String> = requested_skills.into_iter().filter(|s| !is_skip(s)).collect();

    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);

    // Strict Gate: Calling select_skills without confirmation must be blocked with USER_CONFIRMATION_REQUIRED
    if let Some(gate_block) = check_confirmation_gate(&active_skills, is_confirmed, &proposals, &proj_path, state) {
        return Ok(gate_block);
    }

    let task_arg = arguments
        .get("task")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let resp = if active_skills.is_empty() {
        let text = "# Skill Selection\n\nNo skills selected. Proceeding directly to task execution.\n\n-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. Otherwise, answer directly or proceed to task planning.".to_string();
        let tokens = estimate_tokens(&text, false) as u64;
        state.record_call(tokens, tokens);
        text
    } else {
        let all_skills = load_all_skills(&proj_path);
        let mut loaded_sections = Vec::new();
        let mut not_found = Vec::new();
        let mut raw_token_acc: usize = 0;

        for raw_req in &active_skills {
            let clean_name = clean_skill_identifier(raw_req);
            if let Some((canonical_name, content, tag_str)) = resolve_skill(raw_req, &proposals, &all_skills, &proj_path) {
                crate::mcp::db::log_skill_load(&canonical_name);
                raw_token_acc += estimate_tokens(&content, false);
                let processed = if !task_arg.is_empty() && !content.is_empty() {
                    crate::catalog::slicing::slice_skill_markdown(&content, task_arg, 3)
                } else if !content.is_empty() {
                    compress_markdown(&content)
                } else {
                    "*Content empty or unavailable*".to_string()
                };
                loaded_sections.push(format!("### Skill: {}{}\n```markdown\n{}\n```", canonical_name, tag_str, processed));
            } else {
                crate::mcp::db::log_skill_load(&clean_name);
                not_found.push(raw_req.clone());
            }
        }

        let profile = detect_language_fast(&proj_path, task_arg);
        let safety_rules = crate::catalog::slicing::get_language_safety_rules(&profile);

        let mut resp = format!(
            "# Skill Selection Confirmed ({})\n\nLoaded Skills Content:\n\n{}\n\n## Language Safety Rules\n{}",
            loaded_sections.len(),
            loaded_sections.join("\n\n---\n\n"),
            safety_rules
        );

        if !not_found.is_empty() {
            resp.push_str(&format!(
                "\n\nSkills not found:\n{}",
                not_found.iter().map(|n| format!("- {}", n)).collect::<Vec<_>>().join("\n")
            ));
        }

        resp.push_str("\n\n-> NEXT STEP: If codebase inspection is needed, use `project_context(operation=\"search\" | \"read\")`. Otherwise, answer directly or proceed to task planning.");

        let opt_tokens = estimate_tokens(&resp, false);
        let orig_tokens = raw_token_acc.max(opt_tokens);
        state.last_raw_baseline_tokens = Some(orig_tokens as u64);
        state.record_call(orig_tokens as u64, opt_tokens as u64);

        resp
    };
    Ok(resp)
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;