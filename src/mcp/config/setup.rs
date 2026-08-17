use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::mcp::templates::*;
use super::clients::{configure_codex_toml, configure_opencode, merge_mcp_config};
use super::rules::{configure_global_rules, configure_skills_enforcer};

pub fn run_setup(binary_path: &Path) -> Result<()> {
    info!("Configuring MCP clients with binary at {:?}", binary_path);
    let bin_str = binary_path.to_string_lossy().to_string();

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home dir"))?;

    let (claude_path, code_path, cursor_path, devin_path) = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let app_path = PathBuf::from(appdata);
        (
            app_path.join("Claude").join("claude_desktop_config.json"),
            app_path.join("Code").join("User"),
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
            home.join(".config").join("Cursor").join("User"),
            home.join(".config")
                .join("Devin")
                .join("Cascade")
                .join("mcp_config.json"),
        )
    };

    let targets = vec![
        ("Claude Desktop", claude_path, true, "mcpServers"),
        (
            "Antigravity / Gemini Global MCP config",
            home.join(".gemini").join("config").join("mcp_config.json"),
            true,
            "mcpServers",
        ),
        (
            "Antigravity Legacy MCP config",
            home.join(".gemini")
                .join("antigravity")
                .join("mcp_config.json"),
            true,
            "mcpServers",
        ),
        (
            "Cursor Native",
            home.join(".cursor").join("mcp.json"),
            true,
            "mcpServers",
        ),
        (
            "VS Code Native",
            code_path.join("mcp.json"),
            true,
            "servers",
        ),
        (
            "Continue.dev",
            home.join(".continue")
                .join("mcpServers")
                .join("config.json"),
            true,
            "mcpServers",
        ),
        ("Devin/Cascade", devin_path, true, "mcpServers"),
        (
            "Claude Code",
            home.join(".claude").join("mcp.json"),
            true,
            "mcpServers",
        ),
        (
            "Windsurf",
            home.join(".codeium")
                .join("windsurf")
                .join("mcp_config.json"),
            true,
            "mcpServers",
        ),
    ];

    for (name, path, force, key) in targets {
        if force || path.parent().map(|p| p.exists()).unwrap_or(false) {
            merge_mcp_config(&path, SERVER_ID, &bin_str, key)?;
            info!("Successfully configured {}", name);
        }
    }

    let extensions = vec![
        (
            "VS Code Cline",
            code_path
                .join("globalStorage")
                .join("saoudrizwan.claude-dev")
                .join("settings")
                .join("cline_mcp_settings.json"),
        ),
        (
            "VS Code Roo-Code",
            code_path
                .join("globalStorage")
                .join("roovet.roo-cline")
                .join("settings")
                .join("cline_mcp_settings.json"),
        ),
        (
            "Cursor Cline",
            cursor_path
                .join("globalStorage")
                .join("saoudrizwan.claude-dev")
                .join("settings")
                .join("cline_mcp_settings.json"),
        ),
        (
            "Cursor Roo-Code",
            cursor_path
                .join("globalStorage")
                .join("roovet.roo-cline")
                .join("settings")
                .join("cline_mcp_settings.json"),
        ),
    ];

    for (name, path) in extensions {
        if path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.exists())
            .unwrap_or(false)
        {
            merge_mcp_config(&path, SERVER_ID, &bin_str, "mcpServers")?;
            info!("Successfully configured {}", name);
        }
    }

    let opencode_path = home.join(".config").join("opencode").join("opencode.json");
    configure_opencode(&opencode_path, &bin_str)?;

    let codex_path = home.join(".codex").join("config.toml");
    configure_codex_toml(&codex_path, &bin_str)?;

    // Note: Global rules (AGENTS.md/CLAUDE.md) and skills enforcer are intentionally
    // NOT automatically written or overwritten. Users manage their rule files manually.

    println!();
    println!("Pre-downloading ML models for skill search...");
    if let Err(e) = crate::ml::download_models() {
        println!(
            "  ⚠  Model download failed: {}. Models will download on first use.",
            e
        );
    }

    Ok(())
}
