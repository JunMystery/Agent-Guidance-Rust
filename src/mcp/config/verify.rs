use anyhow::Result;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::mcp::templates::*;

use super::clients::check_mcp_registration;

pub fn run_verify_setup(binary_path: &Path) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home dir"))?;
    let bin_str = binary_path.to_string_lossy().to_string();

    println!("=== Agent Guidance Setup Verification ===\n");

    // 1. Check binary
    let exists = binary_path.exists();
    println!(
        "[{}] Binary: {} ({})",
        if exists { "OK" } else { "FAIL" },
        bin_str,
        if exists { "found" } else { "NOT FOUND" }
    );

    let code_user_mcp = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(appdata).join("Code").join("User").join("mcp.json")
    } else if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("Code")
            .join("User")
            .join("mcp.json")
    } else {
        home.join(".config").join("Code").join("User").join("mcp.json")
    };

    let cursor_user_mcp = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(appdata).join("Cursor").join("User").join("mcp.json")
    } else if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("Cursor")
            .join("User")
            .join("mcp.json")
    } else {
        home.join(".config").join("Cursor").join("User").join("mcp.json")
    };

    let cursor_target = if home.join(".cursor").join("mcp.json").exists() {
        home.join(".cursor").join("mcp.json")
    } else {
        cursor_user_mcp
    };

    let claude_desktop_path = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(appdata)
            .join("Claude")
            .join("claude_desktop_config.json")
    } else if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("Claude")
            .join("claude_desktop_config.json")
    } else {
        home.join(".config")
            .join("Claude")
            .join("claude_desktop_config.json")
    };

    let claude_code_target = if home.join(".claude.json").exists() {
        home.join(".claude.json")
    } else {
        home.join(".claude").join("mcp.json")
    };

    // 2. Check MCP configs
    let mcp_targets: Vec<(&str, PathBuf, &str)> = vec![
        (
            "Claude Desktop",
            claude_desktop_path,
            "mcpServers",
        ),
        (
            "Antigravity / Gemini Global",
            home.join(".gemini").join("config").join("mcp_config.json"),
            "mcpServers",
        ),
        (
            "Antigravity Legacy",
            home.join(".gemini")
                .join("antigravity")
                .join("mcp_config.json"),
            "mcpServers",
        ),
        (
            "Cursor",
            cursor_target,
            "mcpServers",
        ),
        (
            "VS Code",
            code_user_mcp,
            "servers",
        ),
        (
            "Continue.dev",
            home.join(".continue")
                .join("mcpServers")
                .join("config.json"),
            "mcpServers",
        ),
        (
            "Devin/Cascade",
            home.join(".config")
                .join("Devin")
                .join("Cascade")
                .join("mcp_config.json"),
            "mcpServers",
        ),
        (
            "Claude Code",
            claude_code_target,
            "mcpServers",
        ),
        (
            "Windsurf",
            home.join(".codeium")
                .join("windsurf")
                .join("mcp_config.json"),
            "mcpServers",
        ),
        (
            "OpenCode",
            home.join(".config").join("opencode").join("opencode.json"),
            "mcp",
        ),
        (
            "ChatGPT / Codex",
            home.join(".codex").join("config.toml"),
            "mcp_servers",
        ),
    ];

    println!("\n--- MCP Client Registrations ---");
    for (name, path, key) in &mcp_targets {
        let registered = check_mcp_registration(path, key);
        println!(
            "[{}] {}: {}",
            if registered { "OK" } else { "  " },
            name,
            path.display()
        );
    }

    // 3. Check global rules
    println!("\n--- Global Rules (AGENTS.md / CLAUDE.md) ---");
    let rule_targets: Vec<(&str, PathBuf)> = vec![
        (
            "Gemini/Antigravity",
            home.join(".gemini").join("config").join("AGENTS.md"),
        ),
        (
            "OpenCode",
            home.join(".config").join("opencode").join("AGENTS.md"),
        ),
        ("Claude Code", home.join(".claude").join("CLAUDE.md")),
        ("ChatGPT/Codex", home.join(".codex").join("AGENTS.md")),
        (
            "Windsurf",
            home.join(".codeium").join("windsurf").join("AGENTS.md"),
        ),
    ];

    for (name, path) in &rule_targets {
        let has_tag = path.exists()
            && fs::read_to_string(path)
                .map(|c| c.contains(AGENT_GUIDANCE_TAG_START))
                .unwrap_or(false);
        println!("[{}] {}", if has_tag { "OK" } else { "  " }, name);
    }

    println!("\n=== Verification Complete ===");
    Ok(())
}
