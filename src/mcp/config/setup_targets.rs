use std::path::{Path, PathBuf};

pub struct McpTarget {
    pub name: &'static str,
    pub path: PathBuf,
    pub force: bool,
    pub key: &'static str,
}

pub struct ExtensionTarget {
    pub name: &'static str,
    pub path: PathBuf,
}

pub fn get_client_targets(home: &Path) -> (Vec<McpTarget>, Vec<ExtensionTarget>) {
    let (claude_path, code_path, code_insiders_path, cursor_path, devin_path) = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let app_path = PathBuf::from(appdata);
        (
            app_path.join("Claude").join("claude_desktop_config.json"),
            app_path.join("Code").join("User"),
            app_path.join("Code - Insiders").join("User"),
            app_path.join("Cursor").join("User"),
            app_path
                .join("Devin")
                .join("Cascade")
                .join("mcp_config.json"),
        )
    } else if cfg!(target_os = "macos") {
        (
            home.join("Library")
                .join("Application Support")
                .join("Claude")
                .join("claude_desktop_config.json"),
            home.join("Library")
                .join("Application Support")
                .join("Code")
                .join("User"),
            home.join("Library")
                .join("Application Support")
                .join("Code - Insiders")
                .join("User"),
            home.join("Library")
                .join("Application Support")
                .join("Cursor")
                .join("User"),
            home.join("Library")
                .join("Application Support")
                .join("Devin")
                .join("Cascade")
                .join("mcp_config.json"),
        )
    } else {
        (
            home.join(".config")
                .join("Claude")
                .join("claude_desktop_config.json"),
            home.join(".config").join("Code").join("User"),
            home.join(".config").join("Code - Insiders").join("User"),
            home.join(".config").join("Cursor").join("User"),
            home.join(".config")
                .join("Devin")
                .join("Cascade")
                .join("mcp_config.json"),
        )
    };

    let targets = vec![
        McpTarget { name: "Claude Desktop", path: claude_path, force: true, key: "mcpServers" },
        McpTarget {
            name: "Antigravity / Gemini Global MCP config",
            path: home.join(".gemini").join("config").join("mcp_config.json"),
            force: true,
            key: "mcpServers",
        },
        McpTarget {
            name: "Antigravity Legacy MCP config",
            path: home.join(".gemini").join("antigravity").join("mcp_config.json"),
            force: true,
            key: "mcpServers",
        },
        McpTarget {
            name: "Cursor Native",
            path: home.join(".cursor").join("mcp.json"),
            force: true,
            key: "mcpServers",
        },
        McpTarget {
            name: "Cursor User Profile",
            path: cursor_path.join("mcp.json"),
            force: false,
            key: "mcpServers",
        },
        McpTarget {
            name: "VS Code Native",
            path: code_path.join("mcp.json"),
            force: true,
            key: "servers",
        },
        McpTarget {
            name: "VS Code Insiders",
            path: code_insiders_path.join("mcp.json"),
            force: false,
            key: "servers",
        },
        McpTarget {
            name: "GitHub Copilot Agent Host",
            path: home.join(".copilot").join("mcp-config.json"),
            force: false,
            key: "mcpServers",
        },
        McpTarget {
            name: "Continue.dev",
            path: home.join(".continue").join("mcpServers").join("config.json"),
            force: true,
            key: "mcpServers",
        },
        McpTarget { name: "Devin/Cascade", path: devin_path, force: true, key: "mcpServers" },
        McpTarget {
            name: "Claude Code",
            path: home.join(".claude.json"),
            force: true,
            key: "mcpServers",
        },
        McpTarget {
            name: "Claude Code Legacy",
            path: home.join(".claude").join("mcp.json"),
            force: false,
            key: "mcpServers",
        },
        McpTarget {
            name: "Windsurf",
            path: home.join(".codeium").join("windsurf").join("mcp_config.json"),
            force: true,
            key: "mcpServers",
        },
    ];

    let extensions = vec![
        ExtensionTarget {
            name: "VS Code Cline",
            path: code_path.join("globalStorage").join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json"),
        },
        ExtensionTarget {
            name: "VS Code Roo-Code",
            path: code_path.join("globalStorage").join("roovet.roo-cline").join("settings").join("cline_mcp_settings.json"),
        },
        ExtensionTarget {
            name: "Cursor Cline",
            path: cursor_path.join("globalStorage").join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json"),
        },
        ExtensionTarget {
            name: "Cursor Roo-Code",
            path: cursor_path.join("globalStorage").join("roovet.roo-cline").join("settings").join("cline_mcp_settings.json"),
        },
    ];

    (targets, extensions)
}
