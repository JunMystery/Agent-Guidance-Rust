use super::*;

#[test]
fn test_session_continuity_learn_and_handoff() {
    let temp_dir = std::env::temp_dir().join(format!("ag_learn_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let mut state = ServerState::new();

    // 1. Record Learning with Category
    let learn_res = handle_tool_call(
        "session_continuity",
        json!({
            "operation": "learn",
            "project_path": temp_dir.to_str().unwrap(),
            "category": "build_test",
            "learning": "Always run cargo test with mock database pool"
        }),
        &mut state,
    );
    assert!(learn_res.is_ok());
    let text = learn_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("Project Learning Saved"));

    // 2. Generate Handoff Protocol
    let handoff_res = handle_tool_call(
        "session_continuity",
        json!({
            "operation": "handoff",
            "project_path": temp_dir.to_str().unwrap(),
            "next_action": "Run cargo test and inspect failure in auth module"
        }),
        &mut state,
    );
    assert!(handoff_res.is_ok());
    let handoff_text = handoff_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(handoff_text.contains("Cross-Agent Handoff Protocol"));

    // 3. Verify handoff file on disk
    let handoff_file = temp_dir.join(".agent-context/handoff.md");
    assert!(handoff_file.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_auto_checkpoint_on_stage_advance_and_edit() {
    let temp_dir = std::env::temp_dir().join(format!("tools_auto_cp_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut state = ServerState::new();

    // 1. Advance to Plan
    let plan_res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "set_stage",
            "target_stage": "Plan",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(plan_res.is_ok());

    let cp_file = temp_dir
        .join(".agent-context")
        .join("sessions")
        .join(format!("{}.json", state.session_id));
    assert!(cp_file.exists(), "Checkpoint should exist after set_stage to Plan");

    // 2. Approve plan
    let app_res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "approve",
            "user_confirmed": true,
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(app_res.is_ok());

    // 3. Advance to Build
    let adv_res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "advance",
            "target_stage": "Build",
            "architecture_pattern": "Layered_Architecture",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(adv_res.is_ok());

    let loaded = ServerState::load_from_dir(&temp_dir).unwrap();
    assert_eq!(loaded.workflow_stage, "Build");
    assert!(loaded.plan_approved);
    assert_eq!(
        loaded.active_architecture_pattern.as_deref(),
        Some("Layered_Architecture")
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_session_diff_and_handoff_integration() {
    let temp_dir = std::env::temp_dir().join(format!("tools_diff_handoff_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut state = ServerState::new();
    state.plan_approved = true;
    state.set_stage("Build").unwrap();

    // 1. Authorize edit on src/lib.rs
    let edit_res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "authorize_edit",
            "relative_path": "src/lib.rs",
            "architecture_pattern": "Layered_Architecture",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(edit_res.is_ok());
    assert_eq!(state.modified_files, vec!["src/lib.rs"]);

    // 2. Call session_continuity(operation="diff")
    let diff_res = handle_tool_call(
        "session_continuity",
        json!({
            "operation": "diff",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(diff_res.is_ok());
    let diff_text = diff_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(diff_text.contains("Session Modification Summary"));
    assert!(diff_text.contains("src/lib.rs"));

    // 3. Call session_continuity(operation="handoff")
    let handoff_res = handle_tool_call(
        "session_continuity",
        json!({
            "operation": "handoff",
            "next_action": "Run cargo test and benchmark",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(handoff_res.is_ok());
    let handoff_text = handoff_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(handoff_text.contains("Cross-Agent Handoff Protocol"));
    assert!(handoff_text.contains("src/lib.rs"));
    assert!(handoff_text.contains("Run cargo test and benchmark"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_session_continuity_list_and_switch() {
    let temp_dir = std::env::temp_dir().join(format!("tools_list_switch_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut target_state = ServerState::with_client_name(Some("cursor".to_string()));
    target_state.workflow_stage = "Build".to_string();
    target_state.plan_approved = true;
    target_state.edit_authorized = true;
    target_state.record_modified_file("src/parser.rs");
    assert!(target_state.save_to_dir(&temp_dir).is_ok());

    let mut active_state = ServerState::with_client_name(Some("antigravity".to_string()));

    // 1. List sessions
    let list_res = handle_tool_call(
        "session_continuity",
        json!({
            "operation": "list",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut active_state,
    );
    assert!(list_res.is_ok());
    let list_text = list_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(list_text.contains("Active & Archived Sessions"));
    assert!(list_text.contains(&target_state.session_id));
    assert!(list_text.contains("cursor"));

    // 2. Switch to target_state.session_id
    let switch_res = handle_tool_call(
        "session_continuity",
        json!({
            "operation": "switch",
            "session_id": target_state.session_id,
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut active_state,
    );
    assert!(switch_res.is_ok());
    let switch_text = switch_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(switch_text.contains("Successfully switched active session"));

    // 3. Verify active_state inherited target session context with Zero-Trust reset
    assert_eq!(active_state.session_id, target_state.session_id);
    assert_eq!(active_state.workflow_stage, "Build");
    assert_eq!(active_state.modified_files, vec!["src/parser.rs"]);
    assert!(!active_state.edit_authorized, "Zero-Trust policy should reset edit_authorized");
    assert!(!active_state.plan_approved, "Zero-Trust policy should reset plan_approved");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
