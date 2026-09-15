use super::*;

#[test]
fn test_workflow_gate_batch_authorize_edit() {
    let mut state = ServerState::new();
    state.plan_approved = true;
    state.set_stage("Build").unwrap();

    let res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "authorize_edit",
            "project_path": ".",
            "relative_paths": ["src/main.rs", "Cargo.toml"],
            "architecture_pattern": "Auto",
            "risk_level": "LOW",
            "justification": "Refactoring test batch"
        }),
        &mut state,
    );

    assert!(res.is_ok(), "Batch authorize_edit must succeed: {:?}", res);
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("Batch Edit Authorization Gate"));
    assert!(text.contains("PASSED (2/2 authorized)"));
    assert!(text.contains("`src/main.rs`"));
    assert!(text.contains("`Cargo.toml`"));
    assert!(state.edit_authorized);
    assert!(state.modified_files.contains(&"src/main.rs".to_string()));
    assert!(state.modified_files.contains(&"Cargo.toml".to_string()));
}

#[test]
fn test_workflow_gate_batch_authorize_with_violations() {
    let mut state = ServerState::new();
    state.plan_approved = true;
    state.set_stage("Build").unwrap();

    let res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "authorize_edit",
            "project_path": ".",
            "relative_paths": ["src/main.rs", "../../../evil.rs"],
            "architecture_pattern": "Auto",
            "risk_level": "LOW",
            "justification": "Testing partial batch"
        }),
        &mut state,
    );

    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("PARTIAL_PASSED (1/2 authorized, 1 blocked)"));
    assert!(text.contains("PATH_TRAVERSAL_PROHIBITED"));
    assert!(state.modified_files.contains(&"src/main.rs".to_string()));
    assert!(!state.modified_files.contains(&"../../../evil.rs".to_string()));
}

#[test]
fn test_workflow_gate_approve_plan_with_batch_preauth() {
    let mut state = ServerState::new();
    assert!(!state.plan_approved);
    assert_eq!(state.workflow_stage, "Context");

    let res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "approve_plan",
            "project_path": ".",
            "user_confirmed": true,
            "relative_paths": ["src/main.rs", "Cargo.toml"],
            "architecture_pattern": "Auto"
        }),
        &mut state,
    );

    assert!(res.is_ok(), "approve_plan with relative_paths must succeed: {:?}", res);
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("approve_plan & pre-authorized"));
    assert!(text.contains("Plan Approved: true"));
    assert_eq!(state.workflow_stage, "Build");
    assert!(state.plan_approved);
    assert!(state.edit_authorized);
    assert!(state.modified_files.contains(&"src/main.rs".to_string()));
}

#[test]
fn test_workflow_gate_batch_empty_array() {
    let mut state = ServerState::new();
    state.plan_approved = true;
    state.set_stage("Build").unwrap();

    let res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "authorize_edit",
            "project_path": ".",
            "relative_paths": [],
            "justification": "Empty array test"
        }),
        &mut state,
    );

    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("NO_FILES_SPECIFIED") || text.contains("RELATIVE_PATH_REQUIRED"));
}

#[test]
fn test_workflow_gate_advance_with_batch_preauth() {
    let mut state = ServerState::new();
    state.set_stage("Plan").unwrap();

    let res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "advance",
            "target_stage": "Build",
            "user_confirmed": true,
            "relative_paths": ["src/main.rs", "Cargo.toml"],
            "architecture_pattern": "Auto",
            "project_path": "."
        }),
        &mut state,
    );

    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("advance & pre-authorized"));
    assert_eq!(state.workflow_stage, "Build");
    assert!(state.plan_approved);
    assert!(state.edit_authorized);
    assert!(state.modified_files.contains(&"src/main.rs".to_string()));
}

