use super::*;

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
            "project_path": tmp_dir.to_str().unwrap(),
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
fn test_guidance_search_ask_question_proposal_and_context_discovery() {
    let mut state = ServerState::new();
    let res = handle_tool_call(
        "guidance",
        json!({
            "operation": "search",
            "workflow": "android compose app architecture",
            "files": ["ui/screens/HomeScreen.kt"]
        }),
        &mut state,
    );
    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("SKILL_PROPOSAL: MANDATORY USER INTERACTION REQUIRED"));
    assert!(text.contains("ask_question"));
    assert!(text.contains("is_multi_select"));
    assert!(text.contains("user_confirmed=true"));
    assert!(!state.pending_skill_proposals.is_empty());
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
    assert!(text.contains("Task Pipeline Activated"));
    assert!(text.contains("Fix sql injection in database queries"));
    assert!(text.contains("Active Phase: implement"));
    assert!(text.contains("Architecture Guidance"));
    assert!(text.contains("Priority Gate: PASSED"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
