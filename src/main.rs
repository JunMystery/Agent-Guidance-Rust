use anyhow::Result;
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod catalog;
mod context;
mod daemon;
mod dashboard;
mod mcp;
mod ml;
mod optimizer;

use mcp::config::{run_setup, run_verify_setup, run_uninstall};
use ml::embeddings::generate_precomputed_cache;

#[cfg(not(unix))]
use daemon::handle_mcp_lines;

#[tokio::main]
async fn main() -> Result<()> {
    // Handle flags that don't need logging
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--dashboard".to_string()) {
        let port = 3000;
        dashboard::run_dashboard_server(port, None)?;
        return Ok(());
    }

    if args.contains(&"--setup".to_string()) {
        let exe_path = env::current_exe()?;
        let target_bin = dirs::home_dir()
            .map(|h| h.join(".local").join("bin").join(if cfg!(windows) { "agent-guidance.exe" } else { "agent-guidance" }))
            .unwrap_or(exe_path);
        run_setup(&target_bin)?;
        println!("Agent Guidance Rust MCP server configured in all IDE clients successfully!");
        return Ok(());
    }

    if args.contains(&"--update".to_string()) || args.contains(&"--auto-update".to_string()) {
        catalog::updater::run_update()?;
        return Ok(());
    }

    if args.contains(&"--verify-setup".to_string()) {
        let exe_path = env::current_exe()?;
        run_verify_setup(&exe_path)?;
        return Ok(());
    }

    if args.contains(&"--uninstall".to_string()) {
        run_uninstall()?;
        println!("Agent Guidance Rust MCP server uninstalled from all IDE clients successfully!");
        return Ok(());
    }

    if args.contains(&"--generate-passage-cache".to_string()) {
        tracing_subscriber::fmt()
            .with_env_filter("info".parse::<tracing_subscriber::EnvFilter>().unwrap())
            .with_writer(std::io::stderr)
            .init();
        generate_precomputed_cache()?;
        println!("✓ Precomputed passage cache written to src/ml/");
        return Ok(());
    }

    if args.contains(&"--session-start".to_string()) || args.contains(&"--re-gate".to_string()) {
        let mut state = mcp::state::ServerState::new();
        state.priority_gate_pass();
        let freshness = state.session_freshness_note();
        let mut msg = "agent-guidance-mcp session started. Priority gate passed and sentinel file created.".to_string();
        if let Some(note) = freshness {
            msg.push_str(&format!(" {}", note));
        }
        let json_payload = serde_json::json!({
            "priority": "INFO",
            "message": msg
        });
        println!("{}", serde_json::to_string(&json_payload)?);
        return Ok(());
    }

    // Initialize logging to stderr (never stdout to avoid corrupting MCP JSON-RPC protocol)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    info!("Starting Agent Guidance MCP Rust Server v{}", env!("CARGO_PKG_VERSION"));

    #[cfg(unix)]
    {
        if args.contains(&"--force-daemon".to_string()) {
            daemon::daemon_main().await;
            return Ok(());
        }
        if args.contains(&"--force-client".to_string()) {
            if !daemon::try_proxy_mode().await {
                eprintln!("No daemon socket found. Is a daemon running?");
                std::process::exit(1);
            }
            return Ok(());
        }

        if daemon::try_proxy_mode().await {
            return Ok(());
        }

        info!("No existing daemon found -- starting in daemon mode.");
        daemon::daemon_main().await;
    }

    #[cfg(not(unix))]
    {
        if args.contains(&"--force-daemon".to_string()) || args.contains(&"--force-client".to_string()) {
            eprintln!("Daemon/proxy mode is not supported on this platform.");
            std::process::exit(1);
        }

        info!("Running in stdio mode (no daemon on this platform).");
        handle_mcp_lines(tokio::io::stdin(), tokio::io::stdout()).await;
    }

    Ok(())
}
