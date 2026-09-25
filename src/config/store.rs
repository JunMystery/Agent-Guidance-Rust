use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use super::schema::AppConfig;

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance"))
        .unwrap_or_else(|| PathBuf::from(".agent-guidance"))
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path();
    let mut config = if path.exists() {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file at {:?}", path))?;
        toml::from_str::<AppConfig>(&content)
            .with_context(|| format!("Failed to parse config file at {:?}", path))?
    } else {
        AppConfig::default()
    };

    apply_env_overrides(&mut config);
    Ok(config)
}

pub fn apply_env_overrides(config: &mut AppConfig) {
    if let Ok(val) = std::env::var("AGENT_GUIDANCE_SERVER_MODE") {
        config.server.mode = val;
    }
    if let Ok(val) = std::env::var("AGENT_GUIDANCE_SERVER_URL") {
        config.server.url = val;
    }
    if let Ok(val) = std::env::var("AGENT_GUIDANCE_API_KEY") {
        config.server.api_key = val;
    }
    if let Ok(val) = std::env::var("AGENT_GUIDANCE_TIMEOUT_MS") {
        if let Ok(parsed) = val.parse::<u64>() {
            config.server.timeout_ms = parsed;
        }
    }
    if let Ok(val) = std::env::var("AGENT_GUIDANCE_DASHBOARD_PORT") {
        if let Ok(parsed) = val.parse::<u16>() {
            config.dashboard.port = parsed;
        }
    }
    if let Ok(val) = std::env::var("AGENT_GUIDANCE_BIND_ADDR") {
        config.dashboard.bind = val;
    }
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let dir = config_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config directory at {:?}", dir))?;
    }

    let toml_str = toml::to_string_pretty(config)
        .context("Failed to serialize config to TOML")?;

    let path = config_path();
    let tmp_path = dir.join("config.toml.tmp");

    fs::write(&tmp_path, toml_str)
        .with_context(|| format!("Failed to write temporary config file at {:?}", tmp_path))?;

    fs::rename(&tmp_path, &path)
        .with_context(|| format!("Failed to atomically rename {:?} to {:?}", tmp_path, path))?;

    info!("Saved configuration to {:?}", path);
    Ok(())
}
