    use super::*;

    #[test]
    fn test_validate_path_traversal() {
        let base = Path::new(".");
        assert!(validate_path(base, "../../../etc/passwd").is_err());
        assert!(validate_path(base, "Cargo.toml").is_ok());
    }

    #[test]
    fn test_guidance_get_local_skill() {
        let mut state = ServerState::new();
        state.plan_approved = true;
        state.set_stage("Build").unwrap();

        let tmp_dir = std::env::temp_dir().join("test_guidance_get");
        let skill_dir = tmp_dir.join(".agents").join("skills").join("custom");
        let _ = std::fs::create_dir_all(&skill_dir);
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_file,
            "---\nname: custom\n---\n# Custom Skill Content",
        )
        .unwrap();

        let res = handle_tool_call(
            "guidance",
            json!({
                "operation": "get",
                "identifier": skill_file.to_string_lossy().to_string()
            }),
            &mut state,
        );

        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("Custom Skill Content"));

        let _ = std::fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_detect_project_path() {
        let state = ServerState::new();
        // 1. Check explicit path
        let explicit = std::env::current_dir().unwrap();
        let res = detect_project_path(&explicit.to_string_lossy(), &state);
        assert_eq!(res, explicit);

        // 2. Check recorded state.project_path memory
        let mut state_recorded = ServerState::new();
        state_recorded.project_path = Some(explicit.to_string_lossy().to_string());
        let res_recorded = detect_project_path(".", &state_recorded);
        assert_eq!(res_recorded, explicit);

        // 3. Check workspace roots
        let mut state2 = ServerState::new();
        state2.workspace_roots = vec![explicit.to_string_lossy().to_string()];
        let res2 = detect_project_path(".", &state2);
        assert_eq!(res2, explicit);

        // 4. Check that process CWD takes priority over stale global_path_file
        let global_path_file = ServerState::global_project_path_file();
        let parent_dir = explicit.parent().unwrap_or(&explicit);
        let _ = std::fs::write(&global_path_file, parent_dir.to_string_lossy().as_bytes());
        let state_global = ServerState::new();
        let res_global = detect_project_path(".", &state_global);
        assert_eq!(res_global, explicit); // Must return process current_dir (explicit), not global_path_file (parent_dir)
        let _ = std::fs::remove_file(&global_path_file);
    }

    #[test]
    fn test_anti_hallucination_verification() {
        let mut state = ServerState::new();
        state.plan_approved = true;
        state.set_stage("Test_Recheck").unwrap();

        let res = handle_tool_call("guidance", json!({ "operation": "verify" }), &mut state);

        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("Anti-Hallucination Post-Code Verification Checklist"));
        assert!(text.contains("User Requirement Alignment"));

        let check_res = handle_tool_call("workflow_gate", json!({ "action": "check" }), &mut state);
        assert!(check_res.is_ok());
        let check_text = check_res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(check_text.contains("ANTI-HALLUCINATION ENFORCER ACTIVE"));
    }

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
    fn test_auto_architecture_detection() {
        let cwd = std::env::current_dir().unwrap();
        let detected = detect_project_architecture(&cwd);
        assert!(matches!(
            detected.as_str(),
            "Clean_Architecture" | "Layered_Architecture" | "Package_By_Feature" | "Orchestrator"
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
    fn test_auto_architecture_gate_authorization() {
        let mut state = ServerState::new();
        state.plan_approved = true;
        state.set_stage("Build").unwrap();

        // 1. Authorize edit with 'Auto' pattern should succeed and resolve pattern
        let res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "authorize_edit",
                "architecture_pattern": "Auto",
                "risk_level": "LOW",
                "justification": "Refactoring test"
            }),
            &mut state,
        );
        assert!(
            res.is_ok(),
            "workflow_gate authorize_edit with 'Auto' must succeed: {:?}",
            res
        );
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("Status: PASSED"));
        assert!(state.edit_authorized);
        assert!(state.active_architecture_pattern.is_some());

        // 2. Query precode guidance should contain active architecture
        let precode_res =
            handle_tool_call("guidance", json!({ "operation": "precode" }), &mut state);
        assert!(precode_res.is_ok());
        let precode_text = precode_res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(precode_text.contains("Architecture Pattern:"));
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
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in unit tests"]
    fn test_task_pipeline_architecture_guidance_output() {
        let mut state = ServerState::new();

        let res = handle_tool_call(
            "task_pipeline",
            json!({ "task": "build new feature", "phase": "plan" }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("## Architecture Guidance"));
        assert!(text.contains("Detected Pattern:"));
    }

    #[test]
    fn test_guidance_workflow_loads_embedded_reference() {
        let mut state = ServerState::new();
        let res = handle_tool_call(
            "guidance",
            json!({ "operation": "workflow", "identifier": "code" }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        // Embedded workflow-code.md should be loaded instead of generic 1-liner
        assert!(text.contains("code") || text.contains("Code") || text.contains("Build"));
        assert!(!text.contains("# Dev Workflow Guidance: [code]\n\nRecommended Flow: Context -> Plan -> Ask/Revise -> Build -> Test/Recheck -> Fix -> Document"));
    }

    #[test]
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in unit tests"]
    fn test_guidance_docs_vector_search() {
        let mut state = ServerState::new();
        let res = handle_tool_call(
            "guidance",
            json!({ "operation": "docs", "query": "rust testing" }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("# Documentation Guidance for"));
    }

    #[test]
    fn test_workflow_gate_plan_approval_via_user_message() {
        let mut state = ServerState::new();
        assert!(!state.plan_approved);

        // 1. Unconfirmed approve_plan without user_message is BLOCKED
        let blocked_res = handle_tool_call(
            "workflow_gate",
            json!({ "action": "approve_plan" }),
            &mut state,
        );
        assert!(blocked_res.is_ok());
        assert!(!state.plan_approved);

        // 2. Calling workflow_gate approve_plan with user_confirmed=true sets plan_approved=true
        let res = handle_tool_call(
            "workflow_gate",
            json!({ "action": "approve_plan", "user_confirmed": true }),
            &mut state,
        );
        assert!(res.is_ok());
        assert!(state.plan_approved);

        // 3. set_stage to Build now succeeds
        let stage_res = handle_tool_call(
            "workflow_gate",
            json!({ "action": "set_stage", "target_stage": "Build" }),
            &mut state,
        );
        assert!(stage_res.is_ok());
        let text = stage_res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("Status: PASSED"));
        assert_eq!(state.workflow_stage, "Build");
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
    fn test_project_context_read_300_loc_warning() {
        let temp_dir = std::env::temp_dir().join(format!("read_loc_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let large_file = temp_dir.join("large_file.rs");
        let content = (1..=350).map(|i| format!("fn function_{}() {{}}", i)).collect::<Vec<_>>().join("\n");
        let _ = std::fs::write(&large_file, &content);

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "project_context",
            json!({
                "operation": "read",
                "relative_path": "large_file.rs",
                "view_mode": "full",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("ARCHITECTURE MANDATE (300 LOC Cap Exceeded)"));
        assert!(text.contains("350 total lines"));
        assert!(text.contains("Decompose into sub-modules upfront"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_target_symbol_extraction_in_large_file() {
        let temp_dir = std::env::temp_dir().join(format!("symbol_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let large_file = temp_dir.join("giant_service.rs");

        // Create a 600-line file with a specific target function at line 450
        let mut lines = Vec::new();
        for i in 1..=440 {
            lines.push(format!("fn dummy_func_{}() {{ let _x = {}; }}", i, i));
        }
        lines.push("pub fn target_critical_function(user_id: u64) -> bool {".to_string());
        lines.push("    let is_valid = user_id > 1000;".to_string());
        lines.push("    println!(\"Processing user: {}\", user_id);".to_string());
        lines.push("    is_valid".to_string());
        lines.push("}".to_string());
        for i in 446..=600 {
            lines.push(format!("fn trailing_func_{}() {{ let _y = {}; }}", i, i));
        }
        let _ = std::fs::write(&large_file, lines.join("\n"));

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "project_context",
            json!({
                "operation": "read",
                "relative_path": "giant_service.rs",
                "target_symbol": "target_critical_function",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );

        assert!(res.is_ok(), "Target symbol extraction failed: {:?}", res);
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Target Symbol Extracted: 'target_critical_function'"), "Output missing target symbol header: {}", text);
        assert!(text.contains("pub fn target_critical_function(user_id: u64) -> bool"), "Output missing function signature: {}", text);
        assert!(text.contains("Processing user:"), "Output missing function body: {}", text);
        assert!(!text.contains("dummy_func_1"), "Output should NOT contain unrelated top functions: {}", text);
        assert!(!text.contains("trailing_func_500"), "Output should NOT contain unrelated bottom functions: {}", text);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_context_deep_search_and_snippets() {
        let temp_dir = std::env::temp_dir().join(format!("deep_search_test_{}", std::process::id()));
        let deep_path = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("ui").join("tour");
        let _ = std::fs::create_dir_all(&deep_path);
        let deep_file = deep_path.join("ProductTour.kt");
        let content = "package com.example.ui.tour\n\nclass ProductTour {\n    fun startTour() {\n        val tourAnchor = 42\n    }\n}\n";
        let _ = std::fs::write(&deep_file, content);

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "project_context",
            json!({
                "operation": "search",
                "query": "touranchor",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok(), "Deep search failed: {:?}", res);
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("ProductTour.kt"), "Deep search did not find ProductTour.kt: {}", text);
        assert!(text.contains("ProductTour") || text.contains("tourAnchor"), "Deep search did not extract symbol/snippet: {}", text);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_context_kotlin_symbols() {
        let temp_dir = std::env::temp_dir().join(format!("kotlin_symbols_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let kt_file = temp_dir.join("ProductTour.kt");
        let content = "package com.nodescape.app.ui.tour\n\nclass ProductTourOverlay {\n    suspend fun showTour() {}\n    companion object {\n        fun create() {}\n    }\n}\n";
        let _ = std::fs::write(&kt_file, content);

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "project_context",
            json!({
                "operation": "symbols",
                "relative_path": "ProductTour.kt",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("class ProductTourOverlay"));
        assert!(text.contains("suspend fun showTour"));
        assert!(text.contains("companion object"));

        let _ = std::fs::remove_dir_all(&temp_dir);
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
    fn test_guidance_precode_kotlin_and_go_rules() {
        let temp_dir = std::env::temp_dir().join(format!("precode_kotlin_test_{}", std::process::id()));
        let kt_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example");
        let _ = std::fs::create_dir_all(&kt_dir);
        let _ = std::fs::write(kt_dir.join("MainActivity.kt"), "class MainActivity");

        let mut state = ServerState::new();
        state.update_project_path(&temp_dir);
        let res = handle_tool_call(
            "guidance",
            json!({
                "operation": "precode",
                "query": "android kotlin UI",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Primary Language: Kotlin/Java"));
        assert!(text.contains("Dispatchers.IO/Default"));
        assert!(text.contains("StateFlow/LiveData lifecycles"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_select_skills_direct_fallback_when_proposals_empty() {
        let mut state = ServerState::new();
        // proposals is empty
        assert!(state.pending_skill_proposals.is_empty());

        let res = handle_tool_call(
            "select_skills",
            json!({
                "skills": ["android-clean-architecture"]
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Skill Selection Confirmed"));
        assert!(text.contains("android-clean-architecture [Embedded Catalog]"));
    }

    #[test]
    #[ignore] // Requires pre-cached HuggingFace model files; avoid network I/O in unit tests
    fn test_task_pipeline_skill_deduplication_and_empty_task_fallback() {
        let mut state = ServerState::new();
        let res = handle_tool_call(
            "task_pipeline",
            json!({
                "task": "",
                "project_path": ".",
                "phase": "plan",
                "focus": "security"
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("# Task Pipeline Activated"));

        // Check that pending_skill_proposals contains no duplicates
        let mut names = std::collections::HashSet::new();
        for (name, _, _) in &state.pending_skill_proposals {
            assert!(names.insert(name.clone()), "Duplicate skill proposal found: {}", name);
        }
    }

    #[test]
    fn test_precode_upfront_split_blueprint() {
        let mut state = ServerState::new();
        state.active_architecture_pattern = Some("CLI_Pipeline".to_string());

        let res = handle_tool_call(
            "guidance",
            json!({ "operation": "precode", "query": "rust" }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Upfront Architecture & 300 LOC Cap (Mandatory)"));
        assert!(text.contains("CLI_Pipeline"));
        assert!(text.contains("CLI entrypoint main (< 80 LOC)"));
    }

    #[test]
    fn test_project_context_cascade_and_learning() {
        let temp_dir = std::env::temp_dir().join(format!("ag_tools_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(temp_dir.join("src"));
        std::fs::write(
            temp_dir.join("src/payment.rs"),
            "pub struct PaymentGateway;\nimpl PaymentGateway {\n    pub fn charge_card(&self) {\n        let timeout = 30;\n    }\n}\n",
        ).unwrap();

        let mut state = ServerState::new();

        // 1. Reindex
        let reindex_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "reindex",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(reindex_res.is_ok());
        let text = reindex_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Project Re-Indexed Successfully"));

        // 2. Search hits symbol FTS
        let search_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "search",
                "project_path": temp_dir.to_str().unwrap(),
                "query": "PaymentGateway"
            }),
            &mut state,
        );
        assert!(search_res.is_ok());
        let text = search_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("PaymentGateway"));

        // 3. Search hits content FTS
        let content_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "search",
                "project_path": temp_dir.to_str().unwrap(),
                "query": "timeout"
            }),
            &mut state,
        );
        assert!(content_res.is_ok());
        let text = content_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("payment.rs"));

        // 4. Learn Alias
        let learn_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "learn_alias",
                "project_path": temp_dir.to_str().unwrap(),
                "alias_term": "thanh toán thẻ",
                "relative_path": "src/payment.rs",
                "resolved_symbol": "PaymentGateway"
            }),
            &mut state,
        );
        assert!(learn_res.is_ok());
        let text = learn_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Alias Learned Successfully"));

        // 5. Search hits Alias Cache
        let alias_search_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "search",
                "project_path": temp_dir.to_str().unwrap(),
                "query": "thanh toán thẻ"
            }),
            &mut state,
        );
        assert!(alias_search_res.is_ok());
        let text = alias_search_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("alias cache"));
        assert!(text.contains("src/payment.rs"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workflow_gate_zero_turn_advance_and_impact_guard() {
        let temp_dir = std::env::temp_dir().join(format!("ag_impact_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(temp_dir.join("src"));
        let core_file = temp_dir.join("src/core.rs");
        std::fs::write(&core_file, "pub struct CoreConfig;\n").unwrap();

        let mut state = ServerState::new();
        state.workflow_stage = "Plan".to_string();
        state.plan_approved = true;

        // 1. Zero-Turn Predictive Transition (Plan -> Build on authorize_edit)
        let auth_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "authorize_edit",
                "project_path": temp_dir.to_str().unwrap(),
                "relative_path": "src/core.rs",
                "architecture_pattern": "CLI_Pipeline",
                "justification": "Modifying core config with unit tests"
            }),
            &mut state,
        );
        assert!(auth_res.is_ok());
        let text = auth_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Status: PASSED"));
        assert_eq!(state.workflow_stage, "Build");
        assert!(state.edit_authorized);

        // 2. Modify file content
        std::fs::write(&core_file, "pub struct CoreConfig;\npub fn new_modified_code() {}\n").unwrap();

        // 3. Rollback Guard restores file
        let rollback_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "rollback",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(rollback_res.is_ok());
        let rollback_text = rollback_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(rollback_text.contains("Successfully restored 1 file(s)"));

        // Verify original content was restored
        let restored_content = std::fs::read_to_string(&core_file).unwrap();
        assert_eq!(restored_content, "pub struct CoreConfig;\n");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in unit tests"]
    fn test_task_pipeline_blueprint_and_recipe() {
        let mut state = ServerState::new();
        let res = handle_tool_call(
            "task_pipeline",
            json!({
                "task": "build payment webhook listener and process transactions",
                "project_path": ".",
                "phase": "plan"
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("## 🍳 Task-Specific Skill Recipe"));
        assert!(text.contains("## 📐 Dynamic Split Blueprint"));
        assert!(text.contains("Upfront Modular Blueprint"));
    }

    #[test]
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in unit tests"]
    fn test_select_skills_semantic_slicing() {
        let mut state = ServerState::new();
        state.pending_skill_proposals = vec![
            ("android-clean-architecture".to_string(), "skills/android-clean-architecture/SKILL.md".to_string(), 0.95),
        ];

        let res = handle_tool_call(
            "select_skills",
            json!({
                "skills": ["android-clean-architecture"],
                "task": "configure domain usecases and repository traits",
                "project_path": "."
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Skill Selection Confirmed"));
        assert!(text.contains("## 🛡️ Language Safety Rules"));
    }

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
    fn test_project_context_skeleton_mode() {
        let temp_dir = std::env::temp_dir().join(format!("ag_skel_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(temp_dir.join("src"));
        let test_file = temp_dir.join("src/large.rs");

        let mut lines = Vec::new();
        lines.push("pub struct BigService;".to_string());
        lines.push("impl BigService {".to_string());
        lines.push("    pub fn heavy_computation(&self) {".to_string());
        for i in 0..320 {
            lines.push(format!("        let var_{} = {};", i, i));
        }
        lines.push("    }".to_string());
        lines.push("}".to_string());

        std::fs::write(&test_file, lines.join("\n")).unwrap();

        let mut state = ServerState::new();

        // Reading file > 300 LOC automatically triggers skeleton mode
        let read_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "read",
                "project_path": temp_dir.to_str().unwrap(),
                "relative_path": "src/large.rs"
            }),
            &mut state,
        );
        assert!(read_res.is_ok());
        let text = read_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("AST Structural Skeleton"));
        assert!(text.contains("pub fn heavy_computation(&self)"));
        assert!(text.contains("Token Saver Mode"));

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

    #[test]
    fn test_guidance_reindex_skills_operation() {
        let temp_dir = std::env::temp_dir().join(format!("reindex_skills_test_{}", std::process::id()));
        let skill_dir = temp_dir.join(".agents").join("skills").join("my-custom-skill");
        let _ = std::fs::create_dir_all(&skill_dir);
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_file,
            "---\nname: my-custom-skill\ndescription: A test custom skill\n---\n# My Custom Skill\n## When to Activate\n- Trigger when testing reindex\n",
        ).unwrap();

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "guidance",
            json!({
                "operation": "reindex_skills",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Skill Semantic Index Refreshed"));
        assert!(text.contains("Catalog Fingerprint:"));
        assert!(text.contains("reindexed with rich semantic passages"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_task_pipeline_enriched_recommendations_formatting() {
        let temp_dir = std::env::temp_dir().join(format!("pipeline_enrich_test_{}", std::process::id()));
        let skill_dir = temp_dir.join(".agents").join("skills").join("sql-safety");
        let _ = std::fs::create_dir_all(&skill_dir);
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_file,
            "---\nname: sql-safety\ndescription: SQL safety and injection defense rules for database queries.\n---\n# SQL Safety\n## Guidelines\n- Always use parameterized queries\n- Never concatenate raw strings\n",
        ).unwrap();

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "task_pipeline",
            json!({
                "task": "Fix sql injection in database queries",
                "project_path": temp_dir.to_str().unwrap(),
                "phase": "implement"
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Recommendations"));
        assert!(text.contains("sql-safety"));
        assert!(text.contains("*Intent*:"));
        assert!(text.contains("*Key Rules*:"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }


