use serde_json::{Value, json};
use std::path::Path;

use crate::catalog::language_detector::detect_language_fast;
use crate::catalog::store::{get_embedded_skill, load_all_skills};
use crate::mcp::state::ServerState;
use crate::optimizer::compressor::{compress_markdown, estimate_tokens};
use super::{detect_project_path, ensure_not_cancelled, validate_path};

fn clean_skill_identifier(raw: &str) -> String {
    let mut s = raw.trim().trim_matches(|c| matches!(c, '`' | '*' | '\'' | '"')).trim();
    while s.starts_with(['-', '*', '•']) {
        s = s[1..].trim();
    }
    if let Some(pos) = s.find(". ") {
        if s[..pos].chars().all(|c| c.is_ascii_digit()) {
            s = s[pos + 2..].trim();
        }
    }
    let cut = [s.find(": "), s.find(" ("), s.find(" [")]
        .into_iter()
        .flatten()
        .min();
    if let Some(idx) = cut {
        s = s[..idx].trim();
    }
    s = s.trim_matches(|c| matches!(c, '`' | '*' | '\'' | '"')).trim();
    let decoded = crate::mcp::state::types::parse_file_uri(s);
    let path = Path::new(decoded.trim());
    if path.file_name().is_some_and(|f| f.eq_ignore_ascii_case("skill.md")) {
        if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|f| f.to_str()) {
            return parent.to_string();
        }
    }
    decoded.trim().to_string()
}

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let requested_skills: Vec<String> = {
        let val = arguments.get("skills").or_else(|| arguments.get("skill"));
        if let Some(arr) = val.and_then(|v| v.as_array()) {
            arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
        } else if let Some(s) = val.and_then(|v| v.as_str()) {
            s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
        } else {
            Vec::new()
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

    // Strict Gate: Calling select_skills without confirmation must be blocked with USER_CONFIRMATION_REQUIRED
    if !active_skills.is_empty() && !is_confirmed {
        let options: Vec<String> = if !proposals.is_empty() {
            proposals.iter().map(|(n, rel, _)| format!("{} ({})", n, rel)).collect()
        } else {
            active_skills.clone()
        };
        if !proposals.is_empty() {
            state.pending_skill_proposals = proposals;
        }
        let question_json = json!({
            "questions": [
                {
                    "question": "Which skills would you like to inject for this task?",
                    "options": options,
                    "is_multi_select": true
                }
            ]
        });
        return Ok(format!(
            "# Skill Selection Gate: [select_skills]\n\nStatus: BLOCKED | Error: USER_CONFIRMATION_REQUIRED: You must NEVER auto-select skills on behalf of the user.\n\nTrigger the IDE tool `ask_question` with `is_multi_select: true`:\n```json\nask_question({})\n```\nOnce the user submits their choice, call `select_skills(skills=[...], user_confirmed=true)`.",
            serde_json::to_string_pretty(&question_json).unwrap_or_default()
        ));
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
            let mut resolved: Option<(String, String, &'static str)> = None;

            // 1. Check proposals
            if let Some((prop_name, rel_path, _)) = proposals.iter().find(|(n, rel, _)| {
                n.eq_ignore_ascii_case(&clean_name)
                    || rel.ends_with(&clean_name)
                    || rel.contains(&format!("/{}", clean_name))
            }) {
                let content = get_embedded_skill(prop_name)
                    .or_else(|| get_embedded_skill(rel_path))
                    .or_else(|| validate_path(&proj_path, rel_path).ok().and_then(|p| std::fs::read_to_string(p).ok()))
                    .or_else(|| validate_path(&proj_path, prop_name).ok().and_then(|p| std::fs::read_to_string(p).ok()));
                if let Some(c) = content {
                    let tag = if get_embedded_skill(prop_name).is_some() || get_embedded_skill(rel_path).is_some() {
                        " [Embedded Catalog]"
                    } else {
                        " [Local Workspace]"
                    };
                    resolved = Some((prop_name.clone(), c, tag));
                }
            }

            // 2. Check all catalog & workspace skills
            if resolved.is_none() {
                if let Some(item) = all_skills.iter().find(|s| {
                    s.name.eq_ignore_ascii_case(&clean_name)
                        || s.relative_path.eq_ignore_ascii_case(&clean_name)
                        || s.relative_path.ends_with(&clean_name)
                        || clean_name.ends_with(&s.relative_path)
                        || clean_name.contains(&format!("/{}/", s.name))
                        || clean_name.contains(&format!("\\{}\\", s.name))
                }) {
                    let tag = match item.source {
                        crate::catalog::store::SkillSource::Embedded => " [Embedded Catalog]",
                        crate::catalog::store::SkillSource::LocalWorkspace(_) => " [Local Workspace]",
                    };
                    resolved = Some((item.name.clone(), item.content.clone(), tag));
                }
            }

            // 3. Fallback to embedded skill direct lookup
            if resolved.is_none() {
                if let Some(c) = get_embedded_skill(&clean_name) {
                    resolved = Some((clean_name.clone(), c, " [Embedded Catalog]"));
                }
            }

            // 4. Fallback to direct file path lookup
            if resolved.is_none() {
                if let Ok(full_path) = validate_path(&proj_path, &clean_name) {
                    if let Ok(c) = std::fs::read_to_string(&full_path) {
                        let name = crate::catalog::store::scanner::extract_frontmatter_name(&c)
                            .unwrap_or_else(|| {
                                full_path
                                    .parent()
                                    .and_then(|p| p.file_name())
                                    .map(|f| f.to_string_lossy().to_string())
                                    .unwrap_or_else(|| clean_name.clone())
                            });
                        resolved = Some((name, c, " [Local Workspace]"));
                    }
                }
            }

            if let Some((canonical_name, content, tag)) = resolved {
                crate::mcp::db::log_skill_load(&canonical_name);
                raw_token_acc += estimate_tokens(&content, false);
                let processed = if !task_arg.is_empty() && !content.is_empty() {
                    crate::catalog::slicing::slice_skill_markdown(&content, task_arg, 3)
                } else if !content.is_empty() {
                    compress_markdown(&content)
                } else {
                    "*Content empty or unavailable*".to_string()
                };
                loaded_sections.push(format!(
                    "### Skill: {}{}\n```markdown\n{}\n```",
                    canonical_name, tag, processed
                ));
            } else {
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