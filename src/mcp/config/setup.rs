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
            "Cursor User Profile",
            cursor_path.join("mcp.json"),
            false,
            "mcpServers",
        ),
        (
            "VS Code Native",
            code_path.join("mcp.json"),
            true,
            "servers",
        ),
        (
            "VS Code Insiders",
            code_insiders_path.join("mcp.json"),
            false,
            "servers",
        ),
        (
            "GitHub Copilot Agent Host",
            home.join(".copilot").join("mcp-config.json"),
            false,
            "mcpServers",
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
            home.join(".claude.json"),
            true,
            "mcpServers",
        ),
        (
            "Claude Code Legacy",
            home.join(".claude").join("mcp.json"),
            false,
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

    // Register directly via VS Code CLI if available
    let vscode_payload = serde_json::json!({
        "name": SERVER_ID,
        "type": "stdio",
        "command": &bin_str,
        "args": []
    })
    .to_string();

    let cli_candidates: &[&[&str]] = if cfg!(target_os = "windows") {
        &[&["code.cmd", "code"], &["code-insiders.cmd", "code-insiders"]]
    } else {
        &[&["code"], &["code-insiders"]]
    };

    for variants in cli_candidates {
        for cmd in *variants {
            if let Ok(output) = std::process::Command::new(cmd)
                .args(["--add-mcp", &vscode_payload])
                .output()
            {
                if output.status.success() {
                    info!("Successfully registered with {} via CLI (--add-mcp)", cmd);
                    break;
                }
            }
        }
    }

    // Register directly via Claude Code CLI if available
    let claude_cmds = if cfg!(target_os = "windows") {
        &["claude.cmd", "claude.exe", "claude"][..]
    } else {
        &["claude"][..]
    };
    for cmd in claude_cmds {
        if let Ok(output) = std::process::Command::new(cmd)
            .args(["mcp", "add", "--scope", "user", SERVER_ID, "--", &bin_str])
            .output()
        {
            if output.status.success() {
                info!("Successfully registered with Claude Code via CLI");
                break;
            }
        }
    }

    // Register directly via Codex CLI if available
    let codex_cmds = if cfg!(target_os = "windows") { &["codex.cmd", "codex.exe", "codex"][..] } else { &["codex"][..] };
    for cmd in codex_cmds {
        if let Ok(output) = std::process::Command::new(cmd).args(["mcp", "add", SERVER_ID, "--", &bin_str]).output() {
            if output.status.success() {
                info!("Successfully registered with Codex via CLI");
                break;
            }
        }
    }

    // Note: Global rules (AGENTS.md/CLAUDE.md) and skills enforcer are intentionally
    // NOT automatically written or overwritten. Users manage their rule files manually.

    println!();
    println!("Pre-downloading ML models for skill search...");
    if let Err(e) = crate::ml::download_models() {
        println!(
            "  Warning: Model download failed: {}. Models will download on first use.",
            e
        );
    }

    Ok(())
}
