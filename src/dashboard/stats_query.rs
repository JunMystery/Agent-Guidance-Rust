use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

use super::stats_query_aggregates::{
    query_hourly_savings, query_phase_and_gov, query_summaries,
};

pub fn query_usage_stats(db_path: &Path, proj_filter: Option<&str>) -> Result<Value> {
    if !db_path.exists() {
        return Ok(json!({ "db_status": "missing" }));
    }

    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;
    ensure_tables(&conn);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let cutoff_24h = now - 86400;

    let is_proj = proj_filter.is_some() && proj_filter != Some("all");
    let p1 = proj_filter.unwrap_or("").replace('\\', "/");
    let p2 = proj_filter.unwrap_or("").replace('/', "\\");

    // 1. Tool breakdown
    let tool_sql = if is_proj {
        "SELECT tool_name, operation, COUNT(*), COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0), COALESCE(AVG(duration_ms), 0)
         FROM tool_calls WHERE started_at >= ?1 AND tool_name != 'mcp_tool' AND (project_path = ?2 COLLATE NOCASE OR project_path = ?3 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?2 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?3 COLLATE NOCASE) GROUP BY tool_name, operation"
    } else {
        "SELECT tool_name, operation, COUNT(*), COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0), COALESCE(AVG(duration_ms), 0)
         FROM tool_calls WHERE started_at >= ?1 AND tool_name != 'mcp_tool' GROUP BY tool_name, operation"
    };
    let mut stmt = conn.prepare(tool_sql)?;
    let tool_breakdown: Vec<Value> = if is_proj {
        stmt.query_map(params![cutoff_24h, p1, p2], map_tool_row)?
    } else {
        stmt.query_map([cutoff_24h], map_tool_row)?
    }
    .filter_map(|r| r.ok())
    .collect();

    // 2. Top Skills
    let mut top_skills_map: HashMap<String, i64> = HashMap::new();
    let mut stmt = conn.prepare("SELECT skill_id, COUNT(*) FROM skill_loads WHERE loaded_at >= ? GROUP BY skill_id")?;
    let rows = stmt.query_map([cutoff_24h], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for item in rows.flatten() {
        if let Some(clean_id) = normalize_skill_name(&item.0) {
            *top_skills_map.entry(clean_id).or_default() += item.1;
        }
    }
    let tc_sql = if is_proj {
        "SELECT operation, COUNT(*) FROM tool_calls WHERE started_at >= ?1 AND (tool_name = 'select_skills' OR tool_name = 'select_skill') AND operation IS NOT NULL AND operation != 'none' AND (project_path = ?2 COLLATE NOCASE OR project_path = ?3 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?2 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?3 COLLATE NOCASE) GROUP BY operation"
    } else {
        "SELECT operation, COUNT(*) FROM tool_calls WHERE started_at >= ?1 AND (tool_name = 'select_skills' OR tool_name = 'select_skill') AND operation IS NOT NULL AND operation != 'none' GROUP BY operation"
    };
    if let Ok(mut tc_stmt) = conn.prepare(tc_sql) {
        let mapper = |r: &rusqlite::Row| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?));
        let tc_rows = if is_proj {
            tc_stmt.query_map(params![cutoff_24h, p1, p2], mapper)
        } else {
            tc_stmt.query_map([cutoff_24h], mapper)
        };
        if let Ok(iter) = tc_rows {
            for (ops, tc_cnt) in iter.flatten() {
                for single in ops.split(',') {
                    if let Some(clean_id) = normalize_skill_name(single) {
                        let e = top_skills_map.entry(clean_id).or_default();
                        *e = (*e).max(tc_cnt);
                    }
                }
            }
        }
    }
    let mut top_skills: Vec<Value> = top_skills_map
        .into_iter()
        .map(|(k, v)| json!({ "skill_id": k, "cnt": v }))
        .collect();
    top_skills.sort_by(|a, b| b["cnt"].as_i64().unwrap_or(0).cmp(&a["cnt"].as_i64().unwrap_or(0)));
    top_skills.truncate(50);

    let mut stmt = conn.prepare("SELECT skill_id, loaded_at FROM skill_loads ORDER BY loaded_at DESC LIMIT 100")?;
    let mut recent_skill_calls: Vec<Value> = Vec::new();
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for item in rows.flatten() {
        if let Some(clean_id) = normalize_skill_name(&item.0) {
            recent_skill_calls.push(json!({ "skill_id": clean_id, "loaded_at": item.1 }));
            if recent_skill_calls.len() >= 50 { break; }
        }
    }
    if recent_skill_calls.len() < 50 {
        let tc_rec = "SELECT operation, started_at FROM tool_calls WHERE (tool_name = 'select_skills' OR tool_name = 'select_skill') AND operation IS NOT NULL AND operation != 'none' ORDER BY started_at DESC LIMIT 50";
        if let Ok(mut stmt) = conn.prepare(tc_rec) {
            if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))) {
                for (op, ts) in rows.flatten() {
                    for single in op.split(',') {
                        if let Some(clean_id) = normalize_skill_name(single) {
                            if !recent_skill_calls.iter().any(|v| v["skill_id"] == clean_id && v["loaded_at"] == ts) {
                                recent_skill_calls.push(json!({ "skill_id": clean_id, "loaded_at": ts }));
                            }
                        }
                    }
                }
                recent_skill_calls.sort_by(|a, b| b["loaded_at"].as_i64().unwrap_or(0).cmp(&a["loaded_at"].as_i64().unwrap_or(0)));
                recent_skill_calls.truncate(50);
            }
        }
    }

    // 3. Recent Actions
    let recent_sql = if is_proj {
        "SELECT tool_name, operation, started_at, duration_ms, tokens_original, tokens_optimized, error_message, target FROM tool_calls WHERE tool_name != 'mcp_tool' AND (project_path = ?1 COLLATE NOCASE OR project_path = ?2 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?1 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?2 COLLATE NOCASE) ORDER BY started_at DESC LIMIT 50"
    } else {
        "SELECT tool_name, operation, started_at, duration_ms, tokens_original, tokens_optimized, error_message, target FROM tool_calls WHERE tool_name != 'mcp_tool' ORDER BY started_at DESC LIMIT 50"
    };
    let mut stmt = conn.prepare(recent_sql)?;
    let recent_actions: Vec<Value> = if is_proj {
        stmt.query_map(params![p1, p2], map_recent_action)?
    } else {
        stmt.query_map([], map_recent_action)?
    }
    .filter_map(|r| r.ok())
    .collect();

    // 4. Hourly Savings & Summaries
    let hourly_savings = query_hourly_savings(&conn, now, is_proj, &p1, &p2);
    let summaries = query_summaries(&conn, now, cutoff_24h, is_proj, &p1, &p2);

    let mut stmt = conn.prepare("SELECT query_text, queried_at FROM embed_queries WHERE queried_at >= ? ORDER BY queried_at DESC LIMIT 50")?;
    let embed_recent: Vec<Value> = stmt.query_map([cutoff_24h], |row| {
        Ok(json!({ "query": row.get::<_, String>(0)?, "created_at": row.get::<_, i64>(1)? }))
    })?.filter_map(|r| r.ok()).collect();

    let past_24h = summaries["past_24h"].clone();
    let (phase_stats, governance_stats) = query_phase_and_gov(&conn, cutoff_24h, is_proj, &p1, &p2);
    Ok(json!({
        "db_status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "project_path": proj_filter.unwrap_or("all"),
        "server_port": crate::dashboard::get_dashboard_port(),
        "totals": past_24h,
        "summaries": summaries,
        "tool_breakdown": tool_breakdown,
        "top_skills": top_skills,
        "recent_skill_calls": recent_skill_calls,
        "recent_actions": recent_actions,
        "hourly_savings": hourly_savings,
        "phase_stats": phase_stats,
        "governance_stats": governance_stats,
        "embed_recent": embed_recent
    }))
}

fn map_tool_row(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "tool_name": row.get::<_, String>(0)?,
        "operation": row.get::<_, Option<String>>(1)?,
        "count": row.get::<_, i64>(2)?,
        "tokens_original": row.get::<_, i64>(3)?,
        "tokens_optimized": row.get::<_, i64>(4)?,
        "avg_duration_ms": row.get::<_, f64>(5)?
    }))
}

fn map_recent_action(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "tool_name": row.get::<_, String>(0)?,
        "operation": row.get::<_, Option<String>>(1)?,
        "started_at": row.get::<_, i64>(2)?,
        "duration_ms": row.get::<_, Option<i64>>(3)?,
        "tokens_original": row.get::<_, Option<i64>>(4)?,
        "tokens_optimized": row.get::<_, Option<i64>>(5)?,
        "error_message": row.get::<_, Option<String>>(6)?,
        "target": row.get::<_, Option<String>>(7)?
    }))
}

fn normalize_skill_name(raw: &str) -> Option<String> {
    let clean = crate::mcp::tools::skills::clean_skill_identifier(raw);
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn ensure_tables(conn: &Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS tool_calls (id INTEGER PRIMARY KEY AUTOINCREMENT, tool_name TEXT NOT NULL, operation TEXT, started_at INTEGER NOT NULL, duration_ms INTEGER, tokens_original INTEGER DEFAULT 0, tokens_optimized INTEGER DEFAULT 0, error_message TEXT, project_path TEXT, target TEXT)", []);
    let _ = conn.execute("ALTER TABLE tool_calls ADD COLUMN target TEXT", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS skill_loads (id INTEGER PRIMARY KEY AUTOINCREMENT, skill_id TEXT NOT NULL, loaded_at INTEGER NOT NULL)", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS embed_queries (id INTEGER PRIMARY KEY AUTOINCREMENT, query_text TEXT NOT NULL, queried_at INTEGER NOT NULL)", []);
    let _ = conn.execute("DELETE FROM tool_calls WHERE tool_name = 'mcp_tool'", []);
}
