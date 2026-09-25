pub mod cleanup;
pub mod setup_server;

use anyhow::Result;
use std::env;

use crate::client::default_client;
use crate::config::{ConfigWatcher, current_config};
use crate::mcp::config::run_setup;

pub fn handle_cli_commands(args: &[String]) -> Result<bool> {
    if args.contains(&"--setup-server".to_string()) {
        return setup_server::run_setup_server_wizard();
    }

    if args.contains(&"--server".to_string()) {
        return handle_server_command(args);
    }

    if let Some(pos) = args.iter().position(|a| a == "--set-server") {
        let target = args.get(pos + 1).cloned().unwrap_or_default();
        return handle_set_server(&target);
    }

    if args.contains(&"--test-server".to_string()) {
        return handle_test_server();
    }

    if args.contains(&"--stats".to_string()) || args.contains(&"--status".to_string()) {
        return handle_stats();
    }

    if args.contains(&"--cleanup".to_string()) {
        return cleanup::handle_cleanup(args);
    }

    if args.contains(&"--reindex-skills".to_string()) {
        let force = args.contains(&"--force".to_string());
        return handle_reindex_skills(force);
    }

    if let Some(pos) = args.iter().position(|a| a == "--delete-skill") {
        let names: Vec<String> = args[pos + 1..]
            .iter()
            .take_while(|a| !a.starts_with("--"))
            .cloned()
            .collect();
        return handle_delete_skill(&names);
    }

    Ok(false)
}

fn handle_server_command(args: &[String]) -> Result<bool> {
    let bind_addr = args
        .iter()
        .position(|a| a == "--bind")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| std::env::var("AGENT_GUIDANCE_BIND_ADDR").ok())
        .unwrap_or_else(|| crate::ml::worker::DEFAULT_BIND_ADDR.to_string());

    let worker_port: u16 = args
        .iter()
        .position(|a| a == "--worker-port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<u16>().ok())
        .or_else(|| {
            std::env::var("AGENT_GUIDANCE_WORKER_PORT")
                .ok()
                .and_then(|s| s.parse::<u16>().ok())
        })
        .unwrap_or(crate::ml::worker::DEFAULT_WORKER_PORT);

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
        .unwrap_or(crate::dashboard::DEFAULT_DASHBOARD_PORT);

    let api_key = args
        .iter()
        .position(|a| a == "--api-key")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| std::env::var("AGENT_GUIDANCE_API_KEY").ok())
        .unwrap_or_default();

    let proj_arg = args
        .iter()
        .position(|a| a == "--project" || a == "-p")
        .and_then(|i| args.get(i + 1))
        .cloned();

    if !args.contains(&"--no-dashboard".to_string()) {
        crate::dashboard::spawn_dashboard_background_bind(&bind_addr, dashboard_port, proj_arg);
    }

    println!("[Server Mode] ML Worker daemon starting on http://{}:{} (dashboard: http://127.0.0.1:{})...", bind_addr, worker_port, dashboard_port);
    crate::ml::worker::run_worker_server(&bind_addr, worker_port, api_key)?;
    Ok(true)
}

fn handle_set_server(target: &str) -> Result<bool> {
    if target.is_empty() {
        eprintln!("Error: Missing URL for --set-server. Example: agent-guidance --set-server http://172.16.11.24:9000 (or 'local')");
        return Ok(true);
    }

    let mut cfg = (*current_config()).clone();
    if target.eq_ignore_ascii_case("local") {
        cfg.server.mode = "local".to_string();
        println!("[OK] Server mode switched to 'local' (standalone execution).");
    } else {
        cfg.server.mode = "remote".to_string();
        cfg.server.url = target.to_string();
        println!("[OK] Server endpoint configured to: {}", target);
    }

    ConfigWatcher::global().update(cfg)?;

    if let Ok(exe) = env::current_exe() {
        println!("Updating IDE MCP client configurations...");
        if let Err(e) = run_setup(&exe) {
            eprintln!("Warning: Failed to update some IDE configurations: {}", e);
        } else {
            println!("[OK] IDE MCP configurations verified across VS Code, Claude Code, Cursor, Windsurf, Zed.");
        }
    }

    Ok(true)
}

fn handle_test_server() -> Result<bool> {
    let cfg = current_config();
    if !cfg.server.is_remote() {
        println!("[NOTICE] Operating in local mode (url: {}).", cfg.server.url);
        println!("Run 'agent-guidance --set-server <URL>' to connect to a remote ML worker.");
        return Ok(true);
    }

    println!("Testing connection to remote ML worker: {} ...", cfg.server.url);
    let client = default_client();
    match client.check_health() {
        Ok((health, latency)) => {
            println!(
                "[OK] Connected to {} | Latency: {:.2?} | Status: {} | Server v{}",
                cfg.server.url, latency, health.status, health.version
            );
            println!(
                "     Engine: {} ({}) | Remote Skills: {} ({} sections)",
                health.engine, health.backend, health.total_skills, health.total_sections
            );
        }
        Err(e) => {
            eprintln!("[FAIL] Connection to {} failed: {}", cfg.server.url, e);
            if cfg.resilience.fallback_to_local {
                println!("[INFO] Client will automatically fall back to local FTS5 keyword search.");
            }
        }
    }
    Ok(true)
}

fn handle_stats() -> Result<bool> {
    let cfg = current_config();
    println!("============================================================");
    println!("  Agent Guidance — System & Skill Registry Status           ");
    println!("============================================================");
    println!("  Client Version:     {}", env!("CARGO_PKG_VERSION"));
    println!("  Active Mode:        {}", if cfg.server.is_remote() { "Remote ML Worker" } else { "Local Standalone" });
    println!("  Configured Server:  {}", cfg.server.url);
    println!("  Fallback to Local:  {}", cfg.resilience.fallback_to_local);
    println!("  Local DB Size:      {:.2} MB", (crate::mcp::db::get_db_size_bytes() as f64) / (1024.0 * 1024.0));

    if cfg.server.is_remote() {
        let client = default_client();
        match client.check_health() {
            Ok((health, latency)) => {
                println!("  Server Status:      OK ({:.2?} ping latency)", latency);
                println!("  Server Version:     {}", health.version);
                println!("  Inference Engine:   {} ({})", health.engine, health.backend);
                if let Ok(stats) = client.get_skill_stats() {
                    println!("  Total Skills:       {}", stats.total_skills);
                    println!("  Total Sections:     {}", stats.total_sections);
                    println!("  Vector Dim:         {}", stats.vectors_dim);
                    println!("  Token Savings:      {:.1}%", stats.token_savings_ratio * 100.0);
                    println!("  Catalog Hash:       {}", &stats.catalog_hash[..stats.catalog_hash.len().min(16)]);
                }
            }
            Err(e) => {
                println!("  Server Status:      OFFLINE ({})", e);
                println!("  Active Fallback:    SQLite FTS5 Local Search");
            }
        }
    }
    println!("============================================================");
    Ok(true)
}

fn handle_reindex_skills(force: bool) -> Result<bool> {
    println!("Reindexing skill registry into binary format (force: {})...", force);
    match crate::ml::embeddings::compiler::compile_staging_to_binary(force) {
        Ok(stats) => {
            println!("[OK] Skills compilation complete in {} ms:", stats.duration_ms);
            println!("  - Total Skills:     {}", stats.total_skills);
            println!("  - Reindexed:        {}", stats.reindexed);
            println!("  - Unchanged:        {}", stats.unchanged_skipped);
            println!("  - Catalog Hash:     {}", stats.catalog_hash);
        }
        Err(e) => {
            eprintln!("[FAIL] Reindexing failed: {}", e);
        }
    }
    Ok(true)
}

fn handle_delete_skill(names: &[String]) -> Result<bool> {
    if names.is_empty() {
        eprintln!("Error: Missing skill name(s) for --delete-skill. Example: agent-guidance --delete-skill skill-a skill-b");
        return Ok(true);
    }
    println!("Deleting {} skill(s) and compacting binary bundle...", names.len());
    match crate::ml::embeddings::compactor::delete_skills_and_compact(names, true) {
        Ok(res) => {
            println!("[OK] Skill deletion and binary compaction successful:");
            println!("  - Deleted Skills:   {}", res.deleted_count);
            println!("  - Remaining Skills: {}", res.remaining_count);
            println!("  - Staging Purged:   {}", res.purged_staging_count);
            if !res.new_catalog_hash.is_empty() {
                println!("  - New Catalog Hash: {}", res.new_catalog_hash);
            }
        }
        Err(e) => {
            eprintln!("[FAIL] Skill deletion failed: {}", e);
        }
    }
    Ok(true)
}

