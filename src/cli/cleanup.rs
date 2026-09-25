use anyhow::Result;

pub fn handle_cleanup(args: &[String]) -> Result<bool> {
    let db_path = dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("usage.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("usage.db"));

    let retention = args
        .iter()
        .position(|a| a == "--retention-days")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(crate::mcp::db::cleanup::DEFAULT_RETENTION_DAYS);

    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    let summary = crate::mcp::db::run_auto_cleanup(&conn, retention)?;
    let db_bytes = crate::mcp::db::get_db_size_bytes();

    let proj_path = args
        .iter()
        .position(|a| a == "--project")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let pruned_snaps = if proj_path.exists() {
        crate::mcp::snapshots::cleanup_stale_snapshots(&proj_path, retention, 20)
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
    Ok(true)
}
