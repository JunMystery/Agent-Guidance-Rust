use super::*;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE tool_calls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_name TEXT NOT NULL,
            operation TEXT,
            started_at INTEGER NOT NULL,
            duration_ms INTEGER,
            tokens_original INTEGER DEFAULT 0,
            tokens_optimized INTEGER DEFAULT 0,
            error_message TEXT,
            project_path TEXT
        );
        CREATE TABLE skill_loads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_id TEXT NOT NULL,
            loaded_at INTEGER NOT NULL
        );
        CREATE TABLE embed_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query_text TEXT NOT NULL,
            queried_at INTEGER NOT NULL
        );
        CREATE TABLE llm_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query_text TEXT NOT NULL,
            queried_at INTEGER NOT NULL
        );
        CREATE TABLE daily_summaries (
            day TEXT PRIMARY KEY,
            tool_calls INTEGER DEFAULT 0,
            skills_loaded INTEGER DEFAULT 0,
            embed_queries INTEGER DEFAULT 0,
            llm_queries INTEGER DEFAULT 0,
            tokens_original INTEGER DEFAULT 0,
            tokens_optimized INTEGER DEFAULT 0
        );
        CREATE TABLE tracked_projects (
            project_path TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            first_seen INTEGER NOT NULL,
            last_active INTEGER NOT NULL,
            total_calls INTEGER DEFAULT 0,
            total_tokens_saved INTEGER DEFAULT 0
        );",
    ).unwrap();
    conn
}

#[test]
fn test_cleanup_ttl_and_column_names() {
    let conn = setup_test_db();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let old_time = now - 10 * 86400; // 10 days ago (expired with 7-day TTL)
    let fresh_time = now - 100;

    // Insert old and fresh tool calls
    conn.execute("INSERT INTO tool_calls (tool_name, started_at) VALUES ('guidance', ?)", [old_time]).unwrap();
    conn.execute("INSERT INTO tool_calls (tool_name, started_at) VALUES ('guidance', ?)", [fresh_time]).unwrap();

    // Insert old and fresh skill loads
    conn.execute("INSERT INTO skill_loads (skill_id, loaded_at) VALUES ('rust', ?)", [old_time]).unwrap();
    conn.execute("INSERT INTO skill_loads (skill_id, loaded_at) VALUES ('rust', ?)", [fresh_time]).unwrap();

    // Insert old and fresh embed & llm queries (testing 'queried_at' column)
    conn.execute("INSERT INTO embed_queries (query_text, queried_at) VALUES ('search', ?)", [old_time]).unwrap();
    conn.execute("INSERT INTO embed_queries (query_text, queried_at) VALUES ('search', ?)", [fresh_time]).unwrap();
    conn.execute("INSERT INTO llm_queries (query_text, queried_at) VALUES ('prompt', ?)", [old_time]).unwrap();
    conn.execute("INSERT INTO llm_queries (query_text, queried_at) VALUES ('prompt', ?)", [fresh_time]).unwrap();

    let summary = run_auto_cleanup(&conn, 7).unwrap();

    assert_eq!(summary.tool_calls_pruned, 1);
    assert_eq!(summary.skill_loads_pruned, 1);
    assert_eq!(summary.embed_queries_pruned, 1);
    assert_eq!(summary.llm_queries_pruned, 1);

    // Verify remaining count is 1 for each
    let tc_count: i64 = conn.query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0)).unwrap();
    let eq_count: i64 = conn.query_row("SELECT COUNT(*) FROM embed_queries", [], |r| r.get(0)).unwrap();
    assert_eq!(tc_count, 1);
    assert_eq!(eq_count, 1);
}

#[test]
fn test_cleanup_daily_summaries_and_dead_projects() {
    let conn = setup_test_db();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let old_active = now - 40 * 86400; // 40 days ago

    // Daily summaries: one ancient, one recent
    conn.execute("INSERT INTO daily_summaries (day) VALUES ('2020-01-01')", []).unwrap();
    let today = format_day_string(now);
    conn.execute("INSERT INTO daily_summaries (day) VALUES (?)", [&today]).unwrap();

    // Tracked project: non-existent path and inactive for > 30 days
    let fake_path = if cfg!(windows) { "C:\\nonexistent_repo_dir_abc123" } else { "/tmp/nonexistent_repo_dir_abc123" };
    conn.execute(
        "INSERT INTO tracked_projects (project_path, project_name, first_seen, last_active) VALUES (?, 'fake', ?, ?)",
        rusqlite::params![fake_path, old_active, old_active],
    ).unwrap();

    let summary = run_auto_cleanup(&conn, 7).unwrap();

    assert_eq!(summary.daily_summaries_pruned, 1);
    assert_eq!(summary.dead_projects_pruned, 1);

    let proj_count: i64 = conn.query_row("SELECT COUNT(*) FROM tracked_projects", [], |r| r.get(0)).unwrap();
    assert_eq!(proj_count, 0);

    let ds_count: i64 = conn.query_row("SELECT COUNT(*) FROM daily_summaries", [], |r| r.get(0)).unwrap();
    assert_eq!(ds_count, 1);
}

#[test]
fn test_cleanup_lru_ceiling() {
    let conn = setup_test_db();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    for i in 0..5 {
        conn.execute(
            "INSERT INTO tool_calls (tool_name, started_at) VALUES ('tool', ?)",
            [now + i],
        ).unwrap();
    }
    let summary = run_auto_cleanup(&conn, 7).unwrap();
    assert_eq!(summary.lru_tool_calls_pruned, 0);
}
