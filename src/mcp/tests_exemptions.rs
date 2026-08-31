#[cfg(test)]
mod tests {
    use crate::mcp::tools::handle_tool_call;
    use crate::mcp::state::ServerState;
    use crate::mcp::tools::gate_edit::is_exempt_from_loc_limit;
    use serde_json::json;

    #[test]
    fn test_is_exempt_from_loc_limit() {
        // Documentation & Markdown
        assert!(is_exempt_from_loc_limit("docs/planning/phases/01_phase_core_dataset.md"));
        assert!(is_exempt_from_loc_limit("README.md"));
        assert!(is_exempt_from_loc_limit("notes.txt"));
        assert!(is_exempt_from_loc_limit("doc.pdf"));

        // Data & Serializations & Configs
        assert!(is_exempt_from_loc_limit("data/dataset.json"));
        assert!(is_exempt_from_loc_limit("dataset.csv"));
        assert!(is_exempt_from_loc_limit("config.yaml"));
        assert!(is_exempt_from_loc_limit("config.toml"));
        assert!(is_exempt_from_loc_limit("Cargo.lock"));
        assert!(is_exempt_from_loc_limit("query.sql"));
        assert!(is_exempt_from_loc_limit(".env"));

        // Assets & Static files
        assert!(is_exempt_from_loc_limit("index.html"));
        assert!(is_exempt_from_loc_limit("style.css"));
        assert!(is_exempt_from_loc_limit("logo.svg"));

        // Source Code files MUST NOT be exempt
        assert!(!is_exempt_from_loc_limit("src/main.rs"));
        assert!(!is_exempt_from_loc_limit("src/services/user_service.py"));
        assert!(!is_exempt_from_loc_limit("src/components/App.tsx"));
        assert!(!is_exempt_from_loc_limit("src/utils.js"));
        assert!(!is_exempt_from_loc_limit("main.go"));
        assert!(!is_exempt_from_loc_limit("Main.java"));
        assert!(!is_exempt_from_loc_limit("server.cpp"));
    }

    #[test]
    fn test_exempt_markdown_file_authorizes_over_300_loc() {
        let temp_dir = std::env::temp_dir().join(format!("doc_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let large_md = temp_dir.join("01_large_plan.md");
        let content = (1..=400).map(|i| format!("# Section {}: Details", i)).collect::<Vec<_>>().join("\n");
        let _ = std::fs::write(&large_md, &content);

        let mut state = ServerState::new();
        state.plan_approved = true;
        let _ = state.set_stage("Build");

        // Authorize edit on large markdown file
        let res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "authorize_edit",
                "relative_path": "01_large_plan.md",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Status: PASSED"));
        assert!(!text.contains("300_LOC_CAP_EXCEEDED"));
        assert!(!text.contains("300 LOC Modular Refactoring Mandate"));

        // Read large markdown file
        let read_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "read",
                "relative_path": "01_large_plan.md",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(read_res.is_ok());
        let read_text = read_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(!read_text.contains("ARCHITECTURE MANDATE (300 LOC Cap Exceeded)"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
