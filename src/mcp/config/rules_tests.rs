#[cfg(test)]
mod tests {
    use crate::mcp::templates::{TargetClient, get_client_enforcer_skill, get_client_rules};

    #[test]
    fn test_vscode_copilot_rules_isolation() {
        let rules = get_client_rules(TargetClient::VSCodeCopilot);
        assert!(rules.contains("textSearch"));
        assert!(rules.contains("readFile"));
        assert!(!rules.contains("view_file"));
        assert!(!rules.contains("codebase_search"));

        let skill = get_client_enforcer_skill(TargetClient::VSCodeCopilot);
        assert!(skill.contains("textSearch"));
        assert!(!skill.contains("view_file"));
    }

    #[test]
    fn test_cursor_rules_isolation() {
        let rules = get_client_rules(TargetClient::Cursor);
        assert!(rules.contains("codebase_search"));
        assert!(rules.contains("read_file"));
        assert!(!rules.contains("textSearch"));
        assert!(!rules.contains("view_file"));

        let skill = get_client_enforcer_skill(TargetClient::Cursor);
        assert!(skill.contains("codebase_search"));
        assert!(!skill.contains("textSearch"));
    }

    #[test]
    fn test_codex_chatgpt_rules_isolation() {
        let rules = get_client_rules(TargetClient::ChatGptCodex);
        assert!(rules.contains("python scripts"));
        assert!(!rules.contains("textSearch"));
        assert!(!rules.contains("view_file"));
        assert!(!rules.contains("codebase_search"));
    }

    #[test]
    fn test_antigravity_rules_isolation() {
        let rules = get_client_rules(TargetClient::Antigravity);
        assert!(rules.contains("view_file"));
        assert!(rules.contains("grep_search"));
        assert!(!rules.contains("textSearch"));
        assert!(!rules.contains("codebase_search"));
    }
}
