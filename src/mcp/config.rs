use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use crate::mcp::templates::*;

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

    Ok(())
}

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
    servers_map.remove(OLD_SERVER_ID);

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
    mcp_map.remove(OLD_SERVER_ID);
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
    // ChatGPT/Codex uses TOML config format at .codex/config.toml
    // Parse existing TOML, add our mcp_servers entry, and write back
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

    // Remove old name if present
    mcp_servers.remove(OLD_SERVER_ID);

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

        let new_content = replace_or_append_tagged_section(&content, AGENT_GUIDANCE_TAG_START, AGENT_GUIDANCE_TAG_END, AGENT_RULES_BLOCK.trim());
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

        let new_content = replace_or_append_tagged_section(&content, AGENT_GUIDANCE_SKILL_TAG_START, AGENT_GUIDANCE_SKILL_TAG_END, ENFORCER_SKILL_CONTENT.trim());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_or_append_tagged_section() {
        let start = "start";
        let end = "end";
        let new_sec = "start content end";

        let res = replace_or_append_tagged_section("", start, end, new_sec);
        assert!(!res.is_empty());
    }
}
