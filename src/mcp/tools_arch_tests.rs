use super::*;

#[test]
fn test_auto_architecture_detection() {
    let cwd = std::env::current_dir().unwrap();
    let detected = detect_project_architecture(&cwd);
    assert!(matches!(
        detected.as_str(),
        "Clean_Architecture"
            | "Layered_Architecture"
            | "Package_By_Feature"
            | "Orchestrator"
            | "CLI_Pipeline"
            | "Flat_Library"
    ));

    let state = ServerState::new();
    let auto_resolved = resolve_architecture_pattern("Auto", &cwd, &state);
    assert_eq!(auto_resolved, detected);

    let empty_resolved = resolve_architecture_pattern("", &cwd, &state);
    assert_eq!(empty_resolved, detected);

    let explicit_resolved = resolve_architecture_pattern("Clean_Architecture", &cwd, &state);
    assert_eq!(explicit_resolved, "Clean_Architecture");
}

#[test]
fn test_project_context_architecture_operation() {
    let mut state = ServerState::new();
    // project_context(operation="architecture") should succeed even in Plan stage
    state.set_stage("Plan").unwrap();

    let res = handle_tool_call(
        "project_context",
        json!({ "operation": "architecture" }),
        &mut state,
    );
    assert!(
        res.is_ok(),
        "project_context operation 'architecture' must succeed in Plan stage: {:?}",
        res
    );
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("# Project Architecture Analysis"));
    assert!(text.contains("Pattern:"));
}

#[test]
fn test_architecture_pattern_persistence_and_locking() {
    let temp_dir = std::env::temp_dir().join(format!("arch_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut state = ServerState::new();

    // 1. Explicitly set architecture pattern via workflow_gate
    let set_res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "set_architecture",
            "architecture_pattern": "CLI_Pipeline",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(set_res.is_ok());
    assert_eq!(
        state.active_architecture_pattern.as_deref(),
        Some("CLI_Pipeline")
    );

    // 2. Verify disk persistence in .agent-context/architecture.json
    let loaded = ServerState::load_persisted_architecture(&temp_dir);
    assert_eq!(loaded.as_deref(), Some("CLI_Pipeline"));

    // 3. Stage transition to Plan does not wipe active_architecture_pattern
    let plan_stage = state.set_stage("Plan");
    assert!(plan_stage.is_ok());
    assert_eq!(
        state.active_architecture_pattern.as_deref(),
        Some("CLI_Pipeline")
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_expanded_architecture_patterns() {
    let mut state = ServerState::new();
    state.workflow_stage = "Build".to_string();
    state.plan_approved = true;

    for pattern in &["CLI_Pipeline", "Flat_Library", "Clean_Architecture", "Layered_Architecture", "Package_By_Feature", "Orchestrator"] {
        let res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "authorize_edit",
                "relative_path": "src/main.rs",
                "architecture_pattern": pattern,
                "risk_level": "LOW",
                "justification": "Testing expanded pattern"
            }),
            &mut state,
        );
        assert!(res.is_ok(), "Pattern {} should be authorized: {:?}", pattern, res);
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Status: PASSED"), "Pattern {} failed authorization: {}", pattern, text);
    }
}

#[test]
fn test_detect_deep_layered_architecture_singular() {
    let temp_dir = std::env::temp_dir().join(format!("deep_arch_layered_{}", std::process::id()));
    let service_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("service");
    let viewmodel_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("viewmodel");
    let _ = std::fs::create_dir_all(&service_dir);
    let _ = std::fs::create_dir_all(&viewmodel_dir);
    let _ = std::fs::write(service_dir.join("PingService.kt"), "class PingService");
    let _ = std::fs::write(viewmodel_dir.join("MainViewModel.kt"), "class MainViewModel");

    let detected = detect_project_architecture(&temp_dir);
    assert_eq!(detected, "Layered_Architecture");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_detect_deep_clean_architecture_infra() {
    let temp_dir = std::env::temp_dir().join(format!("deep_arch_clean_{}", std::process::id()));
    let infra_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("infra");
    let domain_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("domain");
    let _ = std::fs::create_dir_all(&infra_dir);
    let _ = std::fs::create_dir_all(&domain_dir);
    let _ = std::fs::write(infra_dir.join("NetworkClient.kt"), "class NetworkClient");
    let _ = std::fs::write(domain_dir.join("UserEntity.kt"), "class UserEntity");

    let detected = detect_project_architecture(&temp_dir);
    assert_eq!(detected, "Clean_Architecture");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_architectural_decomposition_pattern_guidance() {
    let clean = crate::catalog::blueprint::format_decomposition_guidance("Service.rs", 310, "Clean_Architecture");
    assert!(clean.contains("Clean_Architecture"));
    assert!(clean.contains("domain/"));
    assert!(clean.contains("usecase/"));

    let layered = crate::catalog::blueprint::format_decomposition_guidance("Controller.rs", 350, "Layered_Architecture");
    assert!(layered.contains("Layered_Architecture"));
    assert!(layered.contains("controllers/"));
    assert!(layered.contains("services/"));

    let pbf = crate::catalog::blueprint::format_decomposition_guidance("Feature.rs", 400, "Package_By_Feature");
    assert!(pbf.contains("Package_By_Feature"));
    assert!(pbf.contains("<feature>/handlers"));
    assert!(pbf.contains("<feature>/service"));

    let cli = crate::catalog::blueprint::format_decomposition_guidance("main.rs", 305, "CLI_Pipeline");
    assert!(cli.contains("CLI_Pipeline"));
    assert!(cli.contains("commands/"));
}
