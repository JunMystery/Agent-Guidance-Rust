use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use crate::mcp::templates::*;

const HOOK_SCRIPT: &str = r#"#!/bin/bash
exec agent-guidance --session-start 2>/dev/null
"#;

fn hook_script_name() -> &'static str {
    "agent-guidance-session-start.sh"
}

fn make_claude_hooks_json() -> Value {
    json!({
        "hooks": {
            "SessionStart": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": "agent-guidance --session-start"
                        }
                    ]
                }
            ]
        }
    })
}

fn make_generic_hooks_json() -> Value {
    json!({
        "hooks": {
            "SessionStart": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": "agent-guidance --session-start"
                        }
                    ]
                }
            ]
        }
    })
}

pub fn run_setup(binary_path: &Path) -> Result<()> {
    info!("Configuring MCP clients with binary at {:?}", binary_path);
    let bin_str = binary_path.to_string_lossy().to_string();

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home dir"))?;

    let (claude_path, code_path, cursor_path, devin_path) = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let app_path = PathBuf::from(appdata);
        (
            app_path.join("Claude").join("claude_desktop_config.json"),
            app_path.join("Code").join("User").join("globalStorage"),
            app_path.join("Cursor").join("User").join("globalStorage"),
            app_path.join("Devin").join("Cascade").join("mcp_config.json"),
        )
    } else if cfg!(target_os = "macos") {
        (
            home.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json"),
            home.join("Library").join("Application Support").join("Code").join("User").join("globalStorage"),
            home.join("Library").join("Application Support").join("Cursor").join("User").join("globalStorage"),
            home.join("Library").join("Application Support").join("Devin").join("Cascade").join("mcp_config.json"),
        )
    } else {
        (
            home.join(".config").join("Claude").join("claude_desktop_config.json"),
            home.join(".config").join("Code").join("User").join("globalStorage"),
            home.join(".config").join("Cursor").join("User").join("globalStorage"),
            home.join(".config").join("Devin").join("Cascade").join("mcp_config.json"),
        )
    };

    let targets = vec![
        ("Claude Desktop", claude_path, true, "mcpServers"),
        ("Gemini MCP config", home.join(".gemini").join("config").join("mcp_config.json"), true, "mcpServers"),
        ("Antigravity MCP config", home.join(".gemini").join("antigravity").join("mcp_config.json"), true, "mcpServers"),
        ("Cursor Native", home.join(".cursor").join("mcp.json"), true, "mcpServers"),
        ("VS Code Native", code_path.parent().unwrap_or(&code_path).join("mcp.json"), true, "servers"),
        ("Continue.dev", home.join(".continue").join("mcpServers").join("config.json"), true, "mcpServers"),
        ("Devin/Cascade", devin_path, true, "mcpServers"),
        ("Claude Code", home.join(".claude").join("mcp.json"), true, "mcpServers"),
        ("Windsurf", home.join(".codeium").join("windsurf").join("mcp_config.json"), true, "mcpServers"),
    ];

    for (name, path, force, key) in targets {
        if force || path.parent().map(|p| p.exists()).unwrap_or(false) {
            merge_mcp_config(&path, SERVER_ID, &bin_str, key)?;
            info!("Successfully configured {}", name);
        }
    }

    let extensions = vec![
        ("VS Code Cline", code_path.join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json")),
        ("VS Code Roo-Code", code_path.join("roovet.roo-cline").join("settings").join("cline_mcp_settings.json")),
        ("Cursor Cline", cursor_path.join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json")),
        ("Cursor Roo-Code", cursor_path.join("roovet.roo-cline").join("settings").join("cline_mcp_settings.json")),
    ];

    for (name, path) in extensions {
        if path.parent().and_then(|p| p.parent()).map(|p| p.exists()).unwrap_or(false) {
            merge_mcp_config(&path, SERVER_ID, &bin_str, "mcpServers")?;
            info!("Successfully configured {}", name);
        }
    }

    let opencode_path = home.join(".config").join("opencode").join("opencode.json");
    configure_opencode(&opencode_path, &bin_str)?;

    let codex_path = home.join(".codex").join("config.toml");
    configure_codex_toml(&codex_path, &bin_str)?;

    configure_global_rules(&home)?;
    configure_skills_enforcer(&home)?;
    configure_hooks(&home)?;

    println!();
    println!("Pre-downloading ML models for skill search...");
    if let Err(e) = crate::ml::download_models() {
        println!("  ⚠  Model download failed: {}. Models will download on first use.", e);
    }

    Ok(())
}

pub fn run_verify_setup(binary_path: &Path) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home dir"))?;
    let bin_str = binary_path.to_string_lossy().to_string();

    println!("=== Agent Guidance Setup Verification ===\n");

    // 1. Check binary
    let exists = binary_path.exists();
    println!("[{}] Binary: {} ({})",
        if exists { "✓" } else { "✗" },
        bin_str,
        if exists { "found" } else { "NOT FOUND" }
    );

    // 2. Check MCP configs
    let mcp_targets: Vec<(&str, PathBuf, &str)> = vec![
        ("Claude Desktop", home.join(".config").join("Claude").join("claude_desktop_config.json"), "mcpServers"),
        ("Gemini CLI", home.join(".gemini").join("config").join("mcp_config.json"), "mcpServers"),
        ("Antigravity CLI", home.join(".gemini").join("antigravity").join("mcp_config.json"), "mcpServers"),
        ("Cursor", home.join(".cursor").join("mcp.json"), "mcpServers"),
        ("VS Code", home.join(".config").join("Code").join("User").join("globalStorage").join("mcp.json"), "servers"),
        ("Continue.dev", home.join(".continue").join("mcpServers").join("config.json"), "mcpServers"),
        ("Devin/Cascade", home.join(".config").join("Devin").join("Cascade").join("mcp_config.json"), "mcpServers"),
        ("Claude Code", home.join(".claude").join("mcp.json"), "mcpServers"),
        ("Windsurf", home.join(".codeium").join("windsurf").join("mcp_config.json"), "mcpServers"),
        ("OpenCode", home.join(".config").join("opencode").join("opencode.json"), "mcp"),
    ];

    println!("\n--- MCP Client Registrations ---");
    for (name, path, key) in &mcp_targets {
        let registered = check_mcp_registration(path, key);
        println!("[{}] {}: {}",
            if registered { "✓" } else { " " },
            name,
            path.display()
        );
    }

    // 3. Check hooks
    println!("\n--- Session-Start Hooks ---");
    let hook_targets: Vec<(&str, PathBuf)> = vec![
        ("Claude Code", home.join(".claude").join("hooks.json")),
        ("Antigravity CLI", home.join(".gemini").join("antigravity-cli").join("hooks.json")),
        ("Gemini CLI", home.join(".gemini").join("config").join("hooks.json")),
        ("Cursor", home.join(".cursor").join("hooks.json")),
        ("Cline/Roo-Code", home.join(".agents").join("hooks").join(hook_script_name())),
        ("Windsurf", home.join(".codeium").join("windsurf").join("hooks").join(hook_script_name())),
        ("OpenCode", home.join(".config").join("opencode").join("hooks").join(hook_script_name())),
    ];

    for (name, path) in &hook_targets {
        let present = path.exists();
        println!("[{}] {}: {}",
            if present { "✓" } else { " " },
            name,
            path.display()
        );
    }

    // 4. Check global rules
    println!("\n--- Global Rules (AGENTS.md / CLAUDE.md) ---");
    let rule_targets: Vec<(&str, PathBuf)> = vec![
        ("Gemini/Antigravity", home.join(".gemini").join("config").join("AGENTS.md")),
        ("OpenCode", home.join(".config").join("opencode").join("AGENTS.md")),
        ("Claude Code", home.join(".claude").join("CLAUDE.md")),
        ("ChatGPT/Codex", home.join(".codex").join("AGENTS.md")),
        ("Windsurf", home.join(".codeium").join("windsurf").join("AGENTS.md")),
    ];

    for (name, path) in &rule_targets {
        let has_tag = path.exists() && fs::read_to_string(path)
            .map(|c| c.contains(AGENT_GUIDANCE_TAG_START))
            .unwrap_or(false);
        println!("[{}] {}",
            if has_tag { "✓" } else { " " },
            name
        );
    }

    println!("\n=== Verification Complete ===");
    Ok(())
}

pub fn run_uninstall() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home dir"))?;

    println!("Uninstalling agent-guidance from all IDE clients...");

    let (claude_path, code_path, _cursor_path, devin_path) = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let app_path = PathBuf::from(appdata);
        (
            app_path.join("Claude").join("claude_desktop_config.json"),
            app_path.join("Code").join("User").join("globalStorage"),
            app_path.join("Cursor").join("User").join("globalStorage"),
            app_path.join("Devin").join("Cascade").join("mcp_config.json"),
        )
    } else if cfg!(target_os = "macos") {
        (
            home.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json"),
            home.join("Library").join("Application Support").join("Code").join("User").join("globalStorage"),
            home.join("Library").join("Application Support").join("Cursor").join("User").join("globalStorage"),
            home.join("Library").join("Application Support").join("Devin").join("Cascade").join("mcp_config.json"),
        )
    } else {
        (
            home.join(".config").join("Claude").join("claude_desktop_config.json"),
            home.join(".config").join("Code").join("User").join("globalStorage"),
            home.join(".config").join("Cursor").join("User").join("globalStorage"),
            home.join(".config").join("Devin").join("Cascade").join("mcp_config.json"),
        )
    };

    let mcp_targets: Vec<(PathBuf, &str)> = vec![
        (claude_path, "mcpServers"),
        (home.join(".gemini").join("config").join("mcp_config.json"), "mcpServers"),
        (home.join(".gemini").join("antigravity").join("mcp_config.json"), "mcpServers"),
        (home.join(".cursor").join("mcp.json"), "mcpServers"),
        (code_path.parent().unwrap_or(&code_path).join("mcp.json"), "servers"),
        (home.join(".continue").join("mcpServers").join("config.json"), "mcpServers"),
        (devin_path, "mcpServers"),
        (home.join(".claude").join("mcp.json"), "mcpServers"),
        (home.join(".codeium").join("windsurf").join("mcp_config.json"), "mcpServers"),
        (home.join(".config").join("opencode").join("opencode.json"), "mcp"),
        (home.join(".codex").join("config.toml"), ""),
    ];

    for (path, key) in &mcp_targets {
        if path.exists() {
            if !key.is_empty() {
                remove_mcp_entry(path, SERVER_ID, key)?;
            } else {
                remove_codex_entry(path)?;
            }
            println!("  Removed from: {}", path.display());
        }
    }

    remove_hooks(&home)?;
    remove_global_rules(&home)?;
    remove_skills_enforcer(&home)?;

    println!("\nAgent Guidance uninstalled successfully.");
    Ok(())
}

// ---- Hook Deployment ----

fn configure_hooks(home: &Path) -> Result<()> {
    // 1. Claude Code — hooks.json with SessionStart event
    let claude_hooks = home.join(".claude").join("hooks.json");
    write_json_hooks(&claude_hooks, &make_claude_hooks_json())?;
    info!("Hook deployed: Claude Code");

    // 2. Antigravity CLI — hooks.json with SessionStart event
    let agy_hooks = home.join(".gemini").join("antigravity-cli").join("hooks.json");
    write_json_hooks(&agy_hooks, &make_generic_hooks_json())?;
    info!("Hook deployed: Antigravity CLI");

    // 3. Gemini CLI — hooks.json
    let gemini_hooks = home.join(".gemini").join("config").join("hooks.json");
    write_json_hooks(&gemini_hooks, &make_generic_hooks_json())?;
    info!("Hook deployed: Gemini CLI");

    // 4. Cursor — hooks.json
    let cursor_hooks = home.join(".cursor").join("hooks.json");
    write_json_hooks(&cursor_hooks, &make_generic_hooks_json())?;
    info!("Hook deployed: Cursor");

    // 5. Cline / Roo-Code — script in ~/.agents/hooks/
    let agents_hooks_dir = home.join(".agents").join("hooks");
    let script_path = write_hook_script(&agents_hooks_dir)?;
    info!("Hook deployed: Cline/Roo-Code ({})", script_path.display());

    // 6. Windsurf — script in ~/.codeium/windsurf/hooks/
    let windsurf_hooks_dir = home.join(".codeium").join("windsurf").join("hooks");
    let ws_path = write_hook_script(&windsurf_hooks_dir)?;
    info!("Hook deployed: Windsurf ({})", ws_path.display());

    // 7. OpenCode — script + register in opencode.json
    let opencode_hooks_dir = home.join(".config").join("opencode").join("hooks");
    let oc_path = write_hook_script(&opencode_hooks_dir)?;
    register_opencode_hook(&home.join(".config").join("opencode").join("opencode.json"), &oc_path)?;
    info!("Hook deployed: OpenCode ({})", oc_path.display());

    Ok(())
}

fn write_json_hooks(path: &Path, hooks: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(hooks)?;
    fs::write(path, content)?;
    Ok(())
}

fn write_hook_script(dir: &Path) -> Result<PathBuf> {
    let script_path = dir.join(hook_script_name());
    fs::create_dir_all(dir)?;
    fs::write(&script_path, HOOK_SCRIPT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(script_path)
}

fn register_opencode_hook(config_path: &Path, script_path: &Path) -> Result<()> {
    if !config_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(config_path)?;
    let mut root: Value = serde_json::from_str(&content).unwrap_or_else(|_| json!({}));
    if !root.is_object() {
        root = json!({});
    }
    let obj = root.as_object_mut().expect("Root JSON must be object");
    let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
    if let Some(h) = hooks.as_object_mut() {
        h.insert(
            "SessionStart".to_string(),
            json!(script_path.to_string_lossy().to_string()),
        );
    }
    fs::write(config_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

// ---- Uninstall Helpers ----

fn remove_hooks(home: &Path) -> Result<()> {
    let paths = vec![
        home.join(".claude").join("hooks.json"),
        home.join(".gemini").join("antigravity-cli").join("hooks.json"),
        home.join(".gemini").join("config").join("hooks.json"),
        home.join(".cursor").join("hooks.json"),
        home.join(".agents").join("hooks").join(hook_script_name()),
        home.join(".codeium").join("windsurf").join("hooks").join(hook_script_name()),
        home.join(".config").join("opencode").join("hooks").join(hook_script_name()),
    ];
    for path in &paths {
        if path.exists() {
            fs::remove_file(path)?;
            info!("Removed hook: {}", path.display());
        }
    }
    // Clean up OpenCode hooks registration
    let opencode_cfg = home.join(".config").join("opencode").join("opencode.json");
    if opencode_cfg.exists() {
        if let Ok(content) = fs::read_to_string(&opencode_cfg) {
            if let Ok(mut root) = serde_json::from_str::<Value>(&content) {
                if let Some(obj) = root.as_object_mut() {
                    obj.remove("hooks");
                }
                let _ = fs::write(&opencode_cfg, serde_json::to_string_pretty(&root)?);
            }
        }
    }
    Ok(())
}

fn remove_mcp_entry(config_path: &Path, server_id: &str, key: &str) -> Result<()> {
    let content = fs::read_to_string(config_path)?;
    let mut root: Value = serde_json::from_str(&content).unwrap_or_else(|_| json!({}));
    if !root.is_object() {
        return Ok(());
    }
    if let Some(obj) = root.as_object_mut() {
        if let Some(servers) = obj.get_mut(key).and_then(|v| v.as_object_mut()) {
            servers.remove(server_id);
            servers.remove(crate::mcp::templates::OLD_SERVER_ID);
        }
    }
    fs::write(config_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

fn remove_codex_entry(config_path: &Path) -> Result<()> {
    let content = fs::read_to_string(config_path)?;
    let mut value: toml::Value = content.parse::<toml::Value>().unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
    if let Some(table) = value.as_table_mut() {
        if let Some(servers) = table.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) {
            servers.remove(SERVER_ID);
            servers.remove(OLD_SERVER_ID);
        }
    }
    fs::write(config_path, toml::to_string_pretty(&value)?)?;
    Ok(())
}

fn remove_global_rules(home: &Path) -> Result<()> {
    let targets = vec![
        home.join(".gemini").join("config").join("AGENTS.md"),
        home.join(".config").join("opencode").join("AGENTS.md"),
        home.join(".claude").join("CLAUDE.md"),
        home.join(".codex").join("AGENTS.md"),
        home.join(".codeium").join("windsurf").join("AGENTS.md"),
    ];
    for path in &targets {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                let cleaned = strip_tagged_section(&content, AGENT_GUIDANCE_TAG_START, AGENT_GUIDANCE_TAG_END);
                if cleaned != content {
                    fs::write(path, cleaned)?;
                    info!("Cleaned rules from: {}", path.display());
                }
            }
        }
    }
    Ok(())
}

fn remove_skills_enforcer(home: &Path) -> Result<()> {
    let targets = vec![
        home.join(".claude").join("skills").join("agent-guidance").join("SKILL.md"),
        home.join(".config").join("opencode").join("skills").join("agent-guidance").join("SKILL.md"),
        home.join(".agents").join("skills").join("agent-guidance").join("SKILL.md"),
        home.join(".codex").join("skills").join("agent-guidance").join("SKILL.md"),
        home.join(".codeium").join("windsurf").join("skills").join("agent-guidance").join("SKILL.md"),
    ];
    for path in &targets {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                let cleaned = strip_tagged_section(&content, AGENT_GUIDANCE_SKILL_TAG_START, AGENT_GUIDANCE_SKILL_TAG_END);
                if cleaned.trim().is_empty() {
                    let _ = fs::remove_file(path);
                    info!("Removed skill enforcer: {}", path.display());
                } else if cleaned != content {
                    fs::write(path, cleaned)?;
                    info!("Cleaned skill enforcer: {}", path.display());
                }
            }
        }
    }
    Ok(())
}

fn strip_tagged_section(content: &str, start_tag: &str, end_tag: &str) -> String {
    if let (Some(start_idx), Some(end_idx)) = (content.find(start_tag), content.find(end_tag)) {
        let before = content[..start_idx].trim_end();
        let after = content[end_idx + end_tag.len()..].trim_start();
        let mut res = String::new();
        res.push_str(before);
        if !before.is_empty() && !after.is_empty() {
            res.push('\n');
        }
        res.push_str(after);
        res
    } else {
        content.to_string()
    }
}

// ---- Verification Helpers ----

fn check_mcp_registration(config_path: &Path, key: &str) -> bool {
    if !config_path.exists() {
        return false;
    }
    let content = match fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    match serde_json::from_str::<Value>(&content) {
        Ok(root) => {
            root.get(key)
                .and_then(|v| v.as_object())
                .and_then(|m| m.get(SERVER_ID))
                .is_some()
        }
        Err(_) => content.contains(SERVER_ID),
    }
}

// ---- Original MCP Config Functions (required by run_setup) ----

fn merge_mcp_config(config_path: &Path, server_id: &str, bin_path: &str, key: &str) -> Result<()> {
    if config_path.exists() {
        let bak_path = config_path.with_extension("json.bak");
        let _ = fs::copy(config_path, bak_path);
    }

    let mut root: Value = if config_path.exists() {
        let content = fs::read_to_string(config_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if !root.is_object() {
        root = json!({});
    }

    let obj = root.as_object_mut().expect("Root JSON must be object");
    if !obj.contains_key(key) || !obj[key].is_object() {
        obj.insert(key.to_string(), json!({}));
    }

    let servers_map = obj
        .get_mut(key)
        .expect("Key exists in obj")
        .as_object_mut()
        .expect("Server entry must be object");
    servers_map.remove(crate::mcp::templates::OLD_SERVER_ID);

    servers_map.insert(
        server_id.to_string(),
        json!({
            "command": bin_path,
            "args": []
        }),
    );

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(config_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

fn configure_opencode(opencode_path: &Path, bin_path: &str) -> Result<()> {
    if opencode_path.exists() {
        let bak_path = opencode_path.with_extension("json.bak");
        let _ = fs::copy(opencode_path, bak_path);
    }

    let mut root: Value = if opencode_path.exists() {
        let content = fs::read_to_string(opencode_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if !root.is_object() {
        root = json!({});
    }

    let obj = root.as_object_mut().expect("Root JSON must be object");
    if !obj.contains_key("mcp") || !obj["mcp"].is_object() {
        obj.insert("mcp".to_string(), json!({}));
    }

    let mcp_map = obj
        .get_mut("mcp")
        .expect("mcp exists")
        .as_object_mut()
        .expect("mcp must be object");
    mcp_map.remove(crate::mcp::templates::OLD_SERVER_ID);
    mcp_map.insert(
        SERVER_ID.to_string(),
        json!({
            "type": "local",
            "command": [bin_path],
            "enabled": true,
            "environment": {}
        }),
    );

    let instructions = obj.entry("instructions").or_insert_with(|| json!([]));
    if let Some(arr) = instructions.as_array_mut()
        && !arr.contains(&json!("AGENTS.md")) {
        arr.push(json!("AGENTS.md"));
    }

    if let Some(parent) = opencode_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(opencode_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

fn configure_codex_toml(codex_path: &Path, bin_path: &str) -> Result<()> {
    let existing = if codex_path.exists() {
        fs::read_to_string(codex_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut value: toml::Value = existing.parse::<toml::Value>().unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));

    let table = value.as_table_mut().expect("TOML root must be a table");
    if !table.contains_key("mcp_servers") {
        table.insert("mcp_servers".to_string(), toml::Value::Table(toml::map::Map::new()));
    }

    let mcp_servers = table["mcp_servers"]
        .as_table_mut()
        .expect("mcp_servers must be table");

    mcp_servers.remove(crate::mcp::templates::OLD_SERVER_ID);

    mcp_servers.insert(
        SERVER_ID.to_string(),
        toml::Value::Table({
            let mut s = toml::map::Map::new();
            s.insert("command".to_string(), toml::Value::String(bin_path.to_string()));
            s.insert("args".to_string(), toml::Value::Array(vec![]));
            s
        }),
    );

    if let Some(parent) = codex_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let toml_string = toml::to_string_pretty(&value)?;
    fs::write(codex_path, toml_string)?;
    info!("Successfully configured ChatGPT/Codex");
    Ok(())
}

fn configure_global_rules(home: &Path) -> Result<()> {
    let targets = vec![
        ("Gemini/Antigravity", home.join(".gemini").join("config").join("AGENTS.md")),
        ("OpenCode", home.join(".config").join("opencode").join("AGENTS.md")),
        ("Claude Code Compatibility", home.join(".claude").join("CLAUDE.md")),
        ("ChatGPT/Codex", home.join(".codex").join("AGENTS.md")),
        ("Windsurf", home.join(".codeium").join("windsurf").join("AGENTS.md")),
    ];

    for (_name, path) in targets {
        let content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        let new_content = replace_or_append_tagged_section(&content, AGENT_GUIDANCE_TAG_START, AGENT_GUIDANCE_TAG_END, crate::mcp::templates::AGENT_RULES_BLOCK.trim());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, new_content)?;
    }

    Ok(())
}

fn configure_skills_enforcer(home: &Path) -> Result<()> {
    let global_targets = vec![
        ("Claude Code Global", home.join(".claude").join("skills").join("agent-guidance").join("SKILL.md")),
        ("OpenCode Global", home.join(".config").join("opencode").join("skills").join("agent-guidance").join("SKILL.md")),
        ("Cline/Roo-Code Global", home.join(".agents").join("skills").join("agent-guidance").join("SKILL.md")),
        ("ChatGPT/Codex Global", home.join(".codex").join("skills").join("agent-guidance").join("SKILL.md")),
        ("Windsurf Global", home.join(".codeium").join("windsurf").join("skills").join("agent-guidance").join("SKILL.md")),
    ];

    for (_name, path) in global_targets {
        let content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        let new_content = replace_or_append_tagged_section(&content, AGENT_GUIDANCE_SKILL_TAG_START, AGENT_GUIDANCE_SKILL_TAG_END, crate::mcp::templates::ENFORCER_SKILL_CONTENT.trim());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, new_content)?;
    }

    Ok(())
}

pub fn replace_or_append_tagged_section(content: &str, start_tag: &str, end_tag: &str, new_section: &str) -> String {
    if let (Some(start_idx), Some(end_idx)) = (content.find(start_tag), content.find(end_tag))
        && start_idx < end_idx {
        let before = content[..start_idx].trim_end();
        let after = content[end_idx + end_tag.len()..].trim_start();
        let mut res = String::new();
        res.push_str(before);
        res.push('\n');
        res.push_str(new_section.trim());
        res.push('\n');
        res.push_str(after);
        return res;
    }

    if content.trim().is_empty() {
        new_section.trim().to_string()
    } else {
        let mut res = String::new();
        res.push_str(content.trim());
        res.push('\n');
        res.push('\n');
        res.push_str(new_section.trim());
        res
    }
}
