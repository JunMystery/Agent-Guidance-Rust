use anyhow::Result;
use std::path::Path;
use tracing::info;

use super::clients::{configure_codex_toml, configure_opencode, merge_mcp_config};
use super::setup_cli::register_cli_clients;
use super::setup_targets::get_client_targets;

const SERVER_ID: &str = "agent-guidance";

pub fn run_setup(binary_path: &Path) -> Result<()> {
    info!("Configuring MCP clients with binary at {:?}", binary_path);
    let bin_str = binary_path.to_string_lossy().to_string();

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home dir"))?;
    let (targets, extensions) = get_client_targets(&home);

    for target in targets {
        if target.force || target.path.parent().map(|p| p.exists()).unwrap_or(false) {
            merge_mcp_config(&target.path, SERVER_ID, &bin_str, target.key)?;
            info!("Successfully configured {}", target.name);
        }
    }

    for ext in extensions {
        if ext
            .path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.exists())
            .unwrap_or(false)
        {
            merge_mcp_config(&ext.path, SERVER_ID, &bin_str, "mcpServers")?;
            info!("Successfully configured {}", ext.name);
        }
    }

    let opencode_path = home.join(".config").join("opencode").join("opencode.json");
    configure_opencode(&opencode_path, &bin_str)?;

    let codex_path = home.join(".codex").join("config.toml");
    configure_codex_toml(&codex_path, &bin_str)?;

    register_cli_clients(&bin_str, SERVER_ID);

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
