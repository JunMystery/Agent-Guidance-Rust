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

use mcp::config::{run_setup, run_uninstall, run_upgrade, run_verify_setup};
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
        println!("  --upgrade           Download and install latest release package, update IDE configs");
        println!("  --self-update       Alias for --upgrade");
        println!("  --dashboard         Start real-time web usage dashboard at http://127.0.0.1:3000");
        println!("  --reindex-skills    Precompute and build rich semantic vector index for all skills");
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

    if args.contains(&"--upgrade".to_string()) || args.contains(&"--self-update".to_string()) {
        run_upgrade()?;
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

    if args.contains(&"--build-manifest".to_string()) {
        println!("Building semantic skill index manifest...");
        crate::ml::embeddings::precomputed::generate_manifest_only()?;
        return Ok(());
    }

    if args.contains(&"--reindex-skills".to_string())
        || args.contains(&"--generate-passage-cache".to_string())
    {
        tracing_subscriber::fmt()
            .with_env_filter("info".parse::<tracing_subscriber::EnvFilter>().unwrap())
            .with_writer(std::io::stderr)
            .init();
        let start = std::time::Instant::now();
        println!("============================================================");
        println!("  Agent Guidance — Semantic Skill Indexer & RAG DB Builder  ");
        println!("============================================================");
        generate_precomputed_cache()?;
        let elapsed = start.elapsed();
        println!();
        println!("✓ Semantic skill indexing complete in {:.2?}", elapsed);
        println!("✓ Vector database & manifest saved to ~/.agent-guidance/");
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

    if args.contains(&"--daemon".to_string()) || args.contains(&"--force-daemon".to_string()) {
        daemon::daemon_main().await;
        return Ok(());
    }
    if args.contains(&"--proxy".to_string()) || args.contains(&"--force-client".to_string()) {
        if !daemon::try_proxy_mode().await {
            eprintln!("No daemon socket/pipe found. Is a daemon running?");
            std::process::exit(1);
        }
        return Ok(());
    }

    // Direct, instant, zero-overhead stdio MCP server for all IDEs
    daemon::handle_mcp_lines(tokio::io::stdin(), tokio::io::stdout()).await;
    Ok(())
}
