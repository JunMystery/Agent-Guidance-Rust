use anyhow::Result;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::mcp::templates::*;

pub(crate) fn check_mcp_registration(config_path: &Path, key: &str) -> bool {
    if !config_path.exists() {
        return false;
    }
    let content = match fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    match serde_json::from_str::<Value>(&content) {
        Ok(root) => root
            .get(key)
            .and_then(|v| v.as_object())
            .and_then(|m| m.get(SERVER_ID))
            .is_some(),
        Err(_) => content.contains(SERVER_ID),
    }
}

// ---- Original MCP Config Functions (required by run_setup) ----

pub(crate) fn merge_mcp_config(config_path: &Path, server_id: &str, bin_path: &str, key: &str) -> Result<()> {
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

pub(crate) fn configure_opencode(opencode_path: &Path, bin_path: &str) -> Result<()> {
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

    if let Some(parent) = opencode_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(opencode_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

pub(crate) fn configure_codex_toml(codex_path: &Path, bin_path: &str) -> Result<()> {
    let existing = if codex_path.exists() {
        fs::read_to_string(codex_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut value: toml::Value = existing
        .parse::<toml::Value>()
        .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));

    let table = value.as_table_mut().expect("TOML root must be a table");
    if !table.contains_key("mcp_servers") {
        table.insert(
            "mcp_servers".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );
    }

    let mcp_servers = table["mcp_servers"]
        .as_table_mut()
        .expect("mcp_servers must be table");

    mcp_servers.remove(crate::mcp::templates::OLD_SERVER_ID);

    mcp_servers.insert(
        SERVER_ID.to_string(),
        toml::Value::Table({
            let mut s = toml::map::Map::new();
            s.insert(
                "command".to_string(),
                toml::Value::String(bin_path.to_string()),
            );
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

