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
        if exists { "✓" } else { "✗" },
        bin_str,
        if exists { "found" } else { "NOT FOUND" }
    );

    // 2. Check MCP configs
    let mcp_targets: Vec<(&str, PathBuf, &str)> = vec![
        (
            "Claude Desktop",
            home.join(".config")
                .join("Claude")
                .join("claude_desktop_config.json"),
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
            home.join(".cursor").join("mcp.json"),
            "mcpServers",
        ),
        (
            "VS Code",
            home.join(".config")
                .join("Code")
                .join("User")
                .join("mcp.json"),
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
            home.join(".claude").join("mcp.json"),
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
    ];

    println!("\n--- MCP Client Registrations ---");
    for (name, path, key) in &mcp_targets {
        let registered = check_mcp_registration(path, key);
        println!(
            "[{}] {}: {}",
            if registered { "✓" } else { " " },
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
        println!("[{}] {}", if has_tag { "✓" } else { " " }, name);
    }

    println!("\n=== Verification Complete ===");
    Ok(())
}
