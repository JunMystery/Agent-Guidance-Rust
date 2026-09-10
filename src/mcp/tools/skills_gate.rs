use std::path::Path;
use serde_json::json;
use crate::catalog::store::get_embedded_skill;
use crate::mcp::state::ServerState;
use super::validate_path;

pub(crate) fn clean_skill_identifier(raw: &str) -> String {
    let mut s = raw.trim().trim_matches(|c| matches!(c, '`' | '*' | '\'' | '"')).trim();
    while s.starts_with(['-', '*', '•']) {
        s = s[1..].trim();
    }
    if let Some(pos) = s.find(". ") {
        if pos > 0 && s[..pos].chars().all(|c| c.is_ascii_digit()) {
            s = s[pos + 2..].trim();
        }
    }
    for prefix in &["(recommended)", "[recommended]", "recommended:"] {
        if s.to_ascii_lowercase().starts_with(prefix) {
            s = s[prefix.len()..].trim();
            break;
        }
    }
    let cut = [s.find(": "), s.find(" - "), s.find(" \u{2013} "), s.find(" \u{2014} "), s.find(" ("), s.find(" [")]
        .into_iter()
        .flatten()
        .min();
    if let Some(pos) = cut {
        s = s[..pos].trim();
    }
    s = s.trim_matches(|c| matches!(c, '`' | '*' | '\'' | '"')).trim();
    let clean_path = s.trim_start_matches("file://");
    let path = Path::new(clean_path);
    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
        if file_name.eq_ignore_ascii_case("SKILL.md") || file_name.eq_ignore_ascii_case("SKILL") {
            if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|f| f.to_str()) {
                return parent.to_string();
            }
        } else if clean_path.contains('/') || clean_path.contains('\\') {
            let clean_stem = file_name.trim_end_matches(".md");
            if !clean_stem.is_empty() {
                return clean_stem.to_string();
            }
        }
    }
    s.to_string()
}

pub(crate) fn extract_short_usage(name: &str, rel_path: &str, proj_path: &Path) -> String {
    let content = get_embedded_skill(name).or_else(|| get_embedded_skill(rel_path))
        .or_else(|| validate_path(proj_path, rel_path).ok().and_then(|p| std::fs::read_to_string(p).ok()))
        .or_else(|| validate_path(proj_path, name).ok().and_then(|p| std::fs::read_to_string(p).ok()));

    if let Some(c) = content {
        let mut lines = c.lines();
        while let Some(line) = lines.next() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("description:") {
                let mut val = rest.trim();
                if val == ">" || val == "|" || val.is_empty() {
                    if let Some(next) = lines.next() { val = next.trim(); }
                }
                let cleaned = val.trim_matches(|c| matches!(c, '"' | '\'' | '`')).trim();
                if !cleaned.is_empty() && cleaned != ">" && cleaned != "|" {
                    let first = cleaned.split(". ").next().unwrap_or(cleaned).trim();
                    return if first.len() > 38 {
                        format!("{}...", &first[..35].trim_end())
                    } else {
                        first.to_string()
                    };
                }
            }
        }
    }
    "Skill guidance".to_string()
}

pub(crate) fn check_confirmation_gate(
    active_skills: &[String],
    is_confirmed: bool,
    proposals: &[(String, String, f32)],
    proj_path: &Path,
    state: &mut ServerState,
) -> Option<String> {
    if active_skills.is_empty() || is_confirmed {
        return None;
    }

    let options: Vec<String> = if !proposals.is_empty() {
        proposals.iter().map(|(n, rel, _)| format!("{} - {}", n, extract_short_usage(n, rel, proj_path))).collect()
    } else {
        active_skills.iter().map(|raw| {
            let clean = clean_skill_identifier(raw);
            format!("{} - {}", clean, extract_short_usage(&clean, &clean, proj_path))
        }).collect()
    };
    if !proposals.is_empty() {
        state.pending_skill_proposals = proposals.to_vec();
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
    Some(format!(
        "# Skill Selection Gate: [select_skills]\n\nStatus: BLOCKED | Error: USER_CONFIRMATION_REQUIRED: You must NEVER auto-select skills on behalf of the user.\n\nTrigger the IDE tool `ask_question` with `is_multi_select: true`:\n```json\nask_question({})\n```\nOnce the user submits their choice, call `select_skills(skills=[...], user_confirmed=true)`.",
        serde_json::to_string_pretty(&question_json).unwrap_or_default()
    ))
}
