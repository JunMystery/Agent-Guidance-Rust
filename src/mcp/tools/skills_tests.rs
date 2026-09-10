use super::*;
use crate::mcp::tools::handle_tool_call;
use serde_json::json;

#[test]
fn test_clean_skill_identifier() {
    for (input, expected) in [
        ("agent-guidance", "agent-guidance"),
        ("agent-guidance (e:\\Github\\skills\\agent-guidance\\SKILL.md)", "agent-guidance"),
        ("`android-clean-architecture`", "android-clean-architecture"),
        ("**android-clean-architecture**: Modern Architecture", "android-clean-architecture"),
        ("android-clean-architecture: Guidance for clean architecture (Score: 0.95)", "android-clean-architecture"),
        (".agents/skills/agent-guidance/SKILL.md", "agent-guidance"),
        ("- agent-guidance [Local Workspace] (Score: 0.95)", "agent-guidance"),
        ("1. android-clean-architecture [Embedded] (Score: 0.88)", "android-clean-architecture"),
        ("file:///repo/.agents/skills/security-audit/SKILL.md", "security-audit"),
    ] {
        assert_eq!(clean_skill_identifier(input), expected);
    }
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
            "project_path": temp_dir.to_str().unwrap(),
            "user_confirmed": true
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
            "project_path": temp_dir.to_str().unwrap(),
            "user_confirmed": true
        }),
        &mut state,
    );
    assert!(res2.is_ok(), "select_skills with formatted path failed: {:?}", res2);
    let text2 = res2.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text2.contains("### Skill: custom-audit"));

    // Test string boolean confirmation
    let res3 = handle_tool_call(
        "select_skills",
        json!({
            "skills": ["custom-audit"],
            "project_path": temp_dir.to_str().unwrap(),
            "user_confirmed": "true"
        }),
        &mut state,
    );
    assert!(res3.is_ok(), "select_skills with string boolean failed: {:?}", res3);
    let text3 = res3.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text3.contains("### Skill: custom-audit"));

    // Test skip choices like "None"
    let res4 = handle_tool_call(
        "select_skills",
        json!({
            "skills": ["None"],
            "project_path": temp_dir.to_str().unwrap(),
            "user_confirmed": true
        }),
        &mut state,
    );
    assert!(res4.is_ok(), "select_skills with None failed: {:?}", res4);
    let text4 = res4.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text4.contains("No skills selected"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
