use super::*;

#[test]
fn test_select_skills_flow() {
    let mut state = ServerState::new();
    state.plan_approved = true;
    state.set_stage("Build").unwrap();

    state.pending_skill_proposals = vec![
        (
            "agent-guidance".to_string(),
            "skills/agent-guidance/SKILL.md".to_string(),
            0.95,
        ),
        (
            "test-skill".to_string(),
            "skills/test-skill/SKILL.md".to_string(),
            0.80,
        ),
    ];

    // 1. Unconfirmed selection while proposals exist is BLOCKED
    let blocked_res = handle_tool_call(
        "select_skills",
        json!({ "skills": ["agent-guidance"] }),
        &mut state,
    );
    assert!(blocked_res.is_ok());
    let blocked_text = blocked_res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(blocked_text.contains("USER_CONFIRMATION_REQUIRED"));

    // 2. Confirmed selection succeeds
    let res = handle_tool_call(
        "select_skills",
        json!({ "skills": ["agent-guidance"], "user_confirmed": true }),
        &mut state,
    );
    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("# Skill Selection Confirmed"));
    assert!(text.contains("agent-guidance"));

    // 3. State cleared after selection
    assert!(state.pending_skill_proposals.is_empty());

    // 4. Select with empty array when no proposals remain
    let empty_res = handle_tool_call("select_skills", json!({ "skills": [] }), &mut state);
    assert!(empty_res.is_ok());
    let empty_text = empty_res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(empty_text.contains("No skills selected"));
}

#[test]
fn test_select_skills_direct_fallback_when_proposals_empty() {
    let mut state = ServerState::new();
    // proposals is empty
    assert!(state.pending_skill_proposals.is_empty());

    // 1. Unconfirmed selection even when proposals is empty is strictly BLOCKED
    let blocked_res = handle_tool_call(
        "select_skills",
        json!({
            "skills": ["android-clean-architecture"]
        }),
        &mut state,
    );
    assert!(blocked_res.is_ok());
    let blocked_text = blocked_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(blocked_text.contains("USER_CONFIRMATION_REQUIRED"));
    assert!(blocked_text.contains("ask_question"));

    // 2. Confirmed selection succeeds
    let res = handle_tool_call(
        "select_skills",
        json!({
            "skills": ["android-clean-architecture"],
            "user_confirmed": true
        }),
        &mut state,
    );
    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("Skill Selection Confirmed"));
    assert!(text.contains("android-clean-architecture [Embedded Catalog]"));

    // 3. Autonomous subagent execution succeeds
    let auto_res = handle_tool_call(
        "select_skills",
        json!({
            "skills": ["android-clean-architecture"],
            "autonomous": true
        }),
        &mut state,
    );
    assert!(auto_res.is_ok());
    let auto_text = auto_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(auto_text.contains("Skill Selection Confirmed"));

    // 4. Formatted proposal string with description and score succeeds
    let fmt_res = handle_tool_call(
        "select_skills",
        json!({
            "skills": ["android-clean-architecture: Modern Clean Architecture (Score: 0.95)"],
            "user_confirmed": true
        }),
        &mut state,
    );
    assert!(fmt_res.is_ok());
    let fmt_text = fmt_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(fmt_text.contains("Skill Selection Confirmed"));
    assert!(fmt_text.contains("android-clean-architecture [Embedded Catalog]"));
}
