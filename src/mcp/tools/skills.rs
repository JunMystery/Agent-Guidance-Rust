use serde_json::{Value, json};
use std::path::Path;

use crate::catalog::language_detector::detect_language_fast;
use crate::catalog::store::{get_embedded_skill, load_all_skills};
use crate::mcp::state::ServerState;
use crate::optimizer::compressor::{compress_markdown, estimate_tokens};
use super::{detect_project_path, ensure_not_cancelled, validate_path};

fn clean_skill_identifier(raw: &str) -> String {
    let mut s = raw.trim();
    s = s.trim_matches('`').trim_matches('*').trim_matches('\'').trim_matches('"').trim();

    while s.starts_with('-') || s.starts_with('*') || s.starts_with('•') {
        s = s[1..].trim();
    }
    if let Some(pos) = s.find(". ") {
        if s[..pos].chars().all(|c| c.is_ascii_digit()) {
            s = s[pos + 2..].trim();
        }
    }

    if let Some(idx) = s.find(" (") {
        s = &s[..idx];
    } else if let Some(idx) = s.find(": ") {
        s = &s[..idx];
    }
    let s = s.trim();

    let s = if let Some(idx) = s.find(" [") {
        s[..idx].trim()
    } else {
        s
    };

    let decoded = crate::mcp::state::types::parse_file_uri(s);
    let s = decoded.trim();

    let path = Path::new(s);
    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
        if file_name.eq_ignore_ascii_case("skill.md") {
            if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|f| f.to_str()) {
                return parent.to_string();
            }
        }
    }

    s.to_string()
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
        let all_skills = load_all_skills(&proj_path);
        let mut loaded_sections = Vec::new();
        let mut not_found = Vec::new();
        let mut raw_token_acc: usize = 0;

        for raw_req in &requested_skills {
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
                    .or_else(|| validate_path(&proj_path, prop_name).ok().and_then(|p| std::fs::read_to_string(p).ok()))
                    .unwrap_or_default();
                resolved = Some((prop_name.clone(), content, ""));
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
mod tests {
    use super::*;
    use crate::mcp::tools::handle_tool_call;

    #[test]
    fn test_clean_skill_identifier() {
        assert_eq!(clean_skill_identifier("agent-guidance"), "agent-guidance");
        assert_eq!(
            clean_skill_identifier("agent-guidance (e:\\Github\\skills\\agent-guidance\\SKILL.md)"),
            "agent-guidance"
        );
        assert_eq!(
            clean_skill_identifier("`android-clean-architecture`"),
            "android-clean-architecture"
        );
        assert_eq!(
            clean_skill_identifier(".agents/skills/agent-guidance/SKILL.md"),
            "agent-guidance"
        );
        assert_eq!(
            clean_skill_identifier("- agent-guidance [Local Workspace] (Score: 0.95)"),
            "agent-guidance"
        );
        assert_eq!(
            clean_skill_identifier("1. android-clean-architecture [Embedded] (Score: 0.88)"),
            "android-clean-architecture"
        );
        assert_eq!(
            clean_skill_identifier("file:///repo/.agents/skills/security-audit/SKILL.md"),
            "security-audit"
        );
    }

    #[test]
    fn test_select_skill_singular_alias_and_logging() {
        let temp_dir = std::env::temp_dir().join(format!("select_skill_test_{}", std::process::id()));
        let skill_dir = temp_dir.join(".agents").join("skills").join("custom-audit");
        let _ = std::fs::create_dir_all(&skill_dir);
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_file,
            "---\nname: custom-audit\ndescription: Custom audit rules\n---\n# Custom Audit\nFollow security policies.",
        ).unwrap();

        let mut state = ServerState::new();
        state.update_project_path(&temp_dir);

        let res = handle_tool_call(
            "select_skill",
            json!({
                "skill": "custom-audit",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok(), "select_skill failed: {:?}", res);
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("### Skill: custom-audit"));

        let res2 = handle_tool_call(
            "select_skills",
            json!({
                "skills": [format!("custom-audit ({})", skill_file.display())],
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res2.is_ok(), "select_skills with formatted path failed: {:?}", res2);
        let text2 = res2.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text2.contains("### Skill: custom-audit"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}