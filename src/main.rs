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
        println!("  --dashboard         Start real-time web usage dashboard at http://127.0.0.1:11997");
        println!("  --port, --dashboard-port <PORT> Custom dashboard port (default: 11997)");
        println!("  --project <PATH>    Filter dashboard to a specific project path or name");
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

    if args.contains(&"--cleanup".to_string()) {
        let db_path = dirs::home_dir()
            .map(|h| h.join(".agent-guidance").join("usage.db"))
            .unwrap_or_else(|| std::path::PathBuf::from("usage.db"));

        let retention = args
            .iter()
            .position(|a| a == "--retention-days")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(mcp::db::cleanup::DEFAULT_RETENTION_DAYS);

        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;

        let summary = mcp::db::run_auto_cleanup(&conn, retention)?;
        let db_bytes = mcp::db::get_db_size_bytes();

        let proj_path = args
            .iter()
            .position(|a| a == "--project")
            .and_then(|i| args.get(i + 1))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let pruned_snaps = if proj_path.exists() {
            mcp::snapshots::cleanup_stale_snapshots(&proj_path, retention, 20)
        } else {
            0
        };

        println!("[OK] Auto-Cleanup & Database Vacuum Completed:");
        println!("  - Tool calls pruned: {}", summary.tool_calls_pruned);
        println!("  - Skill loads pruned: {}", summary.skill_loads_pruned);
        println!("  - Queries pruned: {}", summary.embed_queries_pruned + summary.llm_queries_pruned);
        println!("  - Daily summaries pruned: {}", summary.daily_summaries_pruned);
        println!("  - Dead projects pruned: {}", summary.dead_projects_pruned);
        if pruned_snaps > 0 {
            println!("  - Stale project snapshots pruned: {}", pruned_snaps);
        }
        if summary.lru_tool_calls_pruned > 0 {
            println!("  - LRU cap pruned: {}", summary.lru_tool_calls_pruned);
        }
        println!("  - Current DB size on disk: {:.2} MB", db_bytes as f64 / (1024.0 * 1024.0));
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
        daemon::daemon_main(dashboard_port, proj_arg).await;
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
    // 1. Transparently proxy to an existing shared daemon instance if active
    if daemon::try_proxy_mode().await {
        return Ok(());
    }

    // 2. No daemon running -> automatically become the Singleton Shared Daemon Master
    // (serves launching IDE's stdio + opens Named Pipe / Unix Socket for other IDEs)
    daemon::daemon_main(dashboard_port, proj_arg).await;
    Ok(())
}
