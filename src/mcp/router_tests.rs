    use super::*;
    use crate::mcp::state::ServerState;

    #[test]
    fn test_router_resources_list_and_read() {
        let mut state = ServerState::new();

        // 1. Test resources/list
        let list_res = handle_request("resources/list", None, &mut state);
        assert!(list_res.is_ok());
        let list_val = list_res.unwrap();
        let resources = list_val["resources"].as_array().unwrap();
        assert!(resources.len() >= 5);

        // 2. Test resources/read for standards://version
        let read_params = json!({"uri": "standards://version"});
        let read_res = handle_request("resources/read", Some(read_params), &mut state);
        assert!(read_res.is_ok());
        let read_val = read_res.unwrap();
        let contents = &read_val["contents"][0];
        assert_eq!(contents["mimeType"], "application/json");
        assert!(
            contents["text"]
                .as_str()
                .unwrap()
                .contains("Agent Guidance MCP Rust")
        );

        // 3. Test resources/read for agent-guidance-mcp://system/priority
        let read_params = json!({"uri": "agent-guidance-mcp://system/priority"});
        let read_res = handle_request("resources/read", Some(read_params), &mut state);
        assert!(read_res.is_ok());
        let read_val = read_res.unwrap();
        let contents = &read_val["contents"][0];
        assert_eq!(contents["mimeType"], "text/markdown");
        assert!(
            contents["text"]
                .as_str()
                .unwrap()
                .contains("Priority Gate Instructions")
        );
    }
