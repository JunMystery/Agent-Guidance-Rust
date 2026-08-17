#![allow(dead_code, unused_imports)]

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

use mcp::config::{run_setup, run_uninstall, run_verify_setup};
use ml::embeddings::generate_precomputed_cache;

#[cfg(not(unix))]
use daemon::handle_mcp_lines;

#[tokio::main]
async fn main() -> Result<()> {
    // Handle flags that don't need logging
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        println!("Agent Guidance MCP Server & CLI Tool v{}", env!("CARGO_PKG_VERSION"));
        println!("Usage: agent-guidance [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --setup             Install and configure MCP server across all IDE clients");
        println!("  --verify-setup      Verify MCP configuration paths in all IDE clients");
        println!("  --update            Sync and download 3rd-party skill repositories into ~/.agent-guidance/skills");
        println!("  --upgrade           Pull latest source from git, rebuild release binary, and update all IDE configs");
        println!("  --self-update       Alias for --upgrade");
        println!("  --dashboard         Start real-time web usage dashboard at http://127.0.0.1:3000");
        println!("  --uninstall         Remove MCP server configurations from all IDE clients");
        println!("  --help, -h          Print this help message");
        println!();
        println!("When run without flags, agent-guidance runs as an MCP JSON-RPC server on stdio.");
        return Ok(());
    }

    if args.contains(&"--dashboard".to_string()) {
        let port = 3000;
        dashboard::run_dashboard_server(port, None)?;
        return Ok(());
    }

    if args.contains(&"--setup".to_string()) {
        let exe_path = env::current_exe()?;
        let target_bin = if cfg!(windows) {
            dirs::data_local_dir()
                .map(|d| {
                    d.join("Programs")
                        .join("agent-guidance")
                        .join("bin")
                        .join("agent-guidance.exe")
                })
                .unwrap_or(exe_path)
        } else {
            dirs::home_dir()
                .map(|h| h.join(".local").join("bin").join("agent-guidance"))
                .unwrap_or(exe_path)
        };
        run_setup(&target_bin)?;
        println!("Agent Guidance Rust MCP server configured in all IDE clients successfully!");
        return Ok(());
    }

    if args.contains(&"--update".to_string()) || args.contains(&"--auto-update".to_string()) {
        catalog::updater::run_update()?;
        return Ok(());
    }

    if args.contains(&"--upgrade".to_string()) || args.contains(&"--self-update".to_string()) {
        println!("Checking for latest Agent Guidance MCP updates from git...");
        let current_dir = env::current_dir()?;
        let status = std::process::Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(&current_dir)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("✓ Git pull completed. Rebuilding release binary with cargo...");
                let build_status = std::process::Command::new("cargo")
                    .args(["build", "--release"])
                    .current_dir(&current_dir)
                    .status();
                if let Ok(bs) = build_status {
                    if bs.success() {
                        let exe_name = if cfg!(windows) { "agent-guidance.exe" } else { "agent-guidance" };
                        let target_bin = current_dir.join("target").join("release").join(exe_name);
                        run_setup(&target_bin)?;
                        println!("✓ Agent Guidance MCP updated, recompiled, and configured across all IDE clients successfully!");
                    } else {
                        eprintln!("Cargo release build failed.");
                    }
                }
            }
            _ => {
                println!("⚠ Git pull failed or current directory is not a git repository.");
            }
        }
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
        let mut msg =
            "agent-guidance-mcp session started. Priority gate passed and sentinel file created."
                .to_string();
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

    info!(
        "Starting Agent Guidance MCP Rust Server v{}",
        env!("CARGO_PKG_VERSION")
    );

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
        if args.contains(&"--force-daemon".to_string())
            || args.contains(&"--force-client".to_string())
        {
            eprintln!("Daemon/proxy mode is not supported on this platform.");
            std::process::exit(1);
        }

        info!("Running in stdio mode (Windows). Starting background VRAM residency warmup...");
        tokio::task::spawn_blocking(|| {
            let _ = crate::ml::embeddings::eager_vram_warmup();
        });

        handle_mcp_lines(tokio::io::stdin(), tokio::io::stdout()).await;
    }

    Ok(())
}
