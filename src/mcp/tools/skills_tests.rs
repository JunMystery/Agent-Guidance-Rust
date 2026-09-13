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
        ("android-clean-architecture - Modern Clean Architecture guidance", "android-clean-architecture"),
        ("skill-comply - Visualize whether skills, rules, and agent definitions are actually followed", "skill-comply"),
        ("'skill-scout' - Search existing local, marketplace, GitHub, and web skill sources", "skill-scout"),
        ("docker-patterns – Docker and Docker Compose patterns", "docker-patterns"),
        (".agents/skills/agent-guidance/SKILL.md", "agent-guidance"),
        ("- agent-guidance [Local Workspace] (Score: 0.95)", "agent-guidance"),
        ("1. android-clean-architecture [Embedded] (Score: 0.88)", "android-clean-architecture"),
        ("file:///repo/.agents/skills/security-audit/SKILL.md", "security-audit"),
        ("(Recommended) plankton-code-quality - Write-time code quality", "plankton-code-quality"),
        ("[Recommended] plankton-code-quality", "plankton-code-quality"),
        ("Recommended: plankton-code-quality", "plankton-code-quality"),
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

    // Test name alias
    let res_name = handle_tool_call(
        "select_skill",
        json!({
            "name": "custom-audit",
            "project_path": temp_dir.to_str().unwrap(),
            "user_confirmed": true
        }),
        &mut state,
    );
    assert!(res_name.is_ok(), "select_skill with name alias failed: {:?}", res_name);

    // Verify stats query top_skills records custom-audit
    let db_path = crate::mcp::db::get_db_path();
    let stats = crate::dashboard::stats_query::query_usage_stats(&db_path, None).expect("query stats");
    let top = stats["top_skills"].as_array().expect("top skills array");
    assert!(top.iter().any(|s| s["skill_id"] == "custom-audit"), "Top skills must record custom-audit: {:?}", top);

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

#[test]
fn test_select_skills_unconfirmed_proposal_structure() {
    let mut state = ServerState::new();
    state.pending_skill_proposals = vec![
        ("agent-guidance".to_string(), "agent-guidance/SKILL.md".to_string(), 1.5),
    ];

    let res = handle_tool_call(
        "select_skills",
        json!({
            "skills": ["agent-guidance"]
        }),
        &mut state,
    );
    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("USER_CONFIRMATION_REQUIRED"));
    assert!(text.contains("\"is_multi_select\": true"));
    assert!(text.contains("agent-guidance - "));
    assert!(!text.contains("Score:"));
    assert!(!text.contains("agent-guidance ("));
}

#[test]
fn test_unicode_char_boundary_safety() {
    use crate::mcp::tools::helpers::{truncate_bytes_safe, truncate_chars};

    // 1. Em-dash ('—') is 3 bytes (0xE2 0x80 0x94)
    let s_dash = "Readiness checklist for homelab—VLAN and network";
    let trunc_chars = truncate_chars(s_dash, 35);
    assert!(trunc_chars.len() <= s_dash.len());
    // Safe byte truncation exactly inside em-dash
    let em_dash_pos = s_dash.find('—').unwrap();
    let safe_byte = truncate_bytes_safe(s_dash, em_dash_pos + 1);
    assert_eq!(safe_byte.len(), em_dash_pos); // Snaps to char boundary before em-dash

    // 2. Vietnamese sentence from crash log: 'ệ' at byte 59..62
    let vn_task = "Thiết kế game Android 2D Roguelike Landscape, lập docs 4 level";
    let mut state = ServerState::new();
    let res = handle_tool_call(
        "task_pipeline",
        json!({
            "task": vn_task,
            "phase": "plan"
        }),
        &mut state,
    );
    assert!(res.is_ok(), "task_pipeline with Vietnamese unicode failed: {:?}", res);

    // 3. Guidance skill search proposal with unicode descriptions
    let res2 = handle_tool_call(
        "guidance",
        json!({
            "operation": "search",
            "query": "godot game design roguelite"
        }),
        &mut state,
    );
    assert!(res2.is_ok(), "guidance search with unicode failed: {:?}", res2);
    let text = res2.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("ask_question"));
}
