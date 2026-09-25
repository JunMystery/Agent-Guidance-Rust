#![allow(dead_code, unused_imports)]

use anyhow::Result;
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod catalog;
mod cli;
mod client;
mod config;
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
    crate::mcp::mcp_logger::init_mcp_logger();
    let max_threads = std::env::var("AGENT_GUIDANCE_MAX_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(max_threads)
        .thread_name(|i| format!("ag-worker-{i}"))
        .build_global();
    // Handle flags that don't need logging
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("agent-guidance {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.contains(&"--fingerprint".to_string()) || args.contains(&"--verify-signature".to_string()) {
        if let Some(fp) = mcp::fingerprint::BinaryFingerprint::current() {
            fp.print_report();
        } else {
            eprintln!("Error: Unable to resolve executable binary path.");
        }
        return Ok(());
    }
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        println!("Agent Guidance MCP Server & CLI Tool v{}", env!("CARGO_PKG_VERSION"));
        println!("Usage: agent-guidance [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --fingerprint       Print binary provenance, SHA-256 hash, and signature report");
        println!("  --verify-signature  Alias for --fingerprint");
        println!("  --setup             Install and configure MCP server across all IDE clients");
        println!("  --verify-setup      Verify MCP configuration paths in all IDE clients");
        println!("  --upgrade           Download and install latest release package, update IDE configs");
        println!("  --self-update       Alias for --upgrade");
        println!("  --dashboard         Start real-time web usage dashboard at http://127.0.0.1:11997");
        println!("  --port, --dashboard-port <PORT> Custom dashboard port (default: 11997)");
        println!("  --server            Start remote ML worker daemon (default: http://127.0.0.1:11998)");
        println!("  --worker-port <PORT> Custom ML worker port (default: 11998)");
        println!("  --bind <ADDR>       Network bind address (e.g. 0.0.0.0 or 127.0.0.1)");
        println!("  --api-key <KEY>     Bearer authentication token for remote worker");
        println!("  --setup-server      Interactive CLI setup wizard for remote ML worker");
        println!("  --project <PATH>    Filter dashboard to a specific project path or name");
        println!("  --set-server <URL>  Configure remote ML worker endpoint (or 'local' for standalone)");
        println!("  --test-server       Test connection and ping latency to remote ML worker");
        println!("  --stats, --status   Display client mode, system telemetry, and remote skill stats");
        println!("  --prune-missing     Prune deleted/moved projects from usage tracking registry");
        println!("  --cleanup           Auto-clean expired logs, prune dead projects, and vacuum DB");
        println!("  --retention-days <N> Retention window in days for detail logs (default: 7)");
        println!("  --reindex-skills    Precompute and build rich semantic vector index for all skills");
        println!("  --uninstall         Remove MCP server configurations from all IDE clients");
        println!("  --help, -h          Print this help message");
        println!();
        println!("When run without flags, agent-guidance runs as an MCP JSON-RPC server on stdio.");
        return Ok(());
    }

    if cli::handle_cli_commands(&args)? {
        return Ok(());
    }

    if args.contains(&"--prune-missing".to_string()) {
        let db_path = dirs::home_dir()
            .map(|h| h.join(".agent-guidance").join("usage.db"))
            .unwrap_or_else(|| std::path::PathBuf::from("usage.db"));
        let count = dashboard::projects::prune_missing_projects(&db_path)?;
        println!("[OK] Pruned {} missing/deleted projects from registry.", count);
        if !args.contains(&"--dashboard".to_string()) {
            return Ok(());
        }
    }

    let dashboard_port: u16 = args
        .iter()
        .position(|a| a == "--dashboard-port" || a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<u16>().ok())
        .or_else(|| {
            std::env::var("AGENT_GUIDANCE_DASHBOARD_PORT")
                .ok()
                .and_then(|s| s.parse::<u16>().ok())
        })
        .unwrap_or(dashboard::DEFAULT_DASHBOARD_PORT);

    let proj_arg = args
        .iter()
        .position(|a| a == "--project" || a == "-p")
        .and_then(|i| args.get(i + 1))
        .cloned();

    if args.contains(&"--dashboard".to_string()) {
        dashboard::run_dashboard_server(dashboard_port, proj_arg)?;
        return Ok(());
    }

    if args.contains(&"--reindex".to_string()) {
        let proj = proj_arg
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        println!("Reindexing code graph for project: {}", proj.display());
        let mut indexer = context::indexer::IncrementalIndexer::new(&proj)?;
        let report = indexer.full_index()?;
        println!("[OK] Full Index Complete in {} ms:", report.duration_ms);
        println!("  - Files scanned: {}", report.files_scanned);
        println!("  - Files indexed: {}", report.files_indexed);
        println!("  - Files skipped: {}", report.files_skipped);
        println!("  - Symbols extracted: {}", report.symbols_extracted);
        println!("  - Edges created: {}", report.edges_created);
        let _ = indexer.update_graph_rag("Auto");
        println!("  - Communities refreshed: .agent-context/communities.json");
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
        println!("[OK] Semantic skill indexing complete in {:.2?}", elapsed);
        println!("[OK] Vector database and manifest saved to ~/.agent-guidance/");
        return Ok(());
    }


    if args.contains(&"--session-start".to_string()) || args.contains(&"--re-gate".to_string()) {
        let mut state = mcp::state::ServerState::new();
        state.priority_gate_pass();
        let freshness = state.session_freshness_note();
        let mut msg =
            "agent-guidance session started. Priority gate passed and sentinel file created."
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

    let is_mcp_server_mode = args.len() == 1
        || args.contains(&"--daemon".to_string())
        || args.contains(&"--force-daemon".to_string())
        || args.contains(&"--proxy".to_string())
        || args.contains(&"--force-client".to_string());

    if !is_mcp_server_mode {
        eprintln!("Unknown argument(s): {:?}", &args[1..]);
        eprintln!("Run 'agent-guidance --help' for usage.");
        std::process::exit(1);
    }

    info!(
        "Starting Agent Guidance MCP Rust Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    if args.contains(&"--daemon".to_string()) || args.contains(&"--force-daemon".to_string()) {
        daemon::server::daemon_main(dashboard_port, proj_arg).await;
        return Ok(());
    }
    if args.contains(&"--proxy".to_string()) || args.contains(&"--force-client".to_string()) {
        if !daemon::try_proxy_mode().await {
            eprintln!("No daemon socket/pipe found. Is a daemon running?");
            std::process::exit(1);
        }
        return Ok(());
    }

    // Auto-Negotiation for Zero-Friction Singleton Shared Daemon Architecture:
    // 1. Transparently proxy to an existing shared daemon instance or auto-spawn a detached one
    if daemon::try_proxy_mode().await {
        return Ok(());
    }
    // 2. Fallback: if proxy mode could not connect or spawn daemon, run direct stdio server
    tracing::warn!("Could not connect to shared daemon — falling back to direct stdio server.");
    daemon::handle_mcp_lines(tokio::io::stdin(), tokio::io::stdout()).await;
    Ok(())
}
