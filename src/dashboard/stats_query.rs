use anyhow::Result;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn query_usage_stats(db_path: &PathBuf, proj_filter: Option<&str>) -> Result<Value> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    ensure_tables(&conn);

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let cutoff_24h = now - 86400;

    let filter_proj = proj_filter.filter(|p| *p != "all" && !p.trim().is_empty());
    let (p1, p2) = match filter_proj {
        Some(p) => {
            let clean = p.trim().trim_end_matches(['/', '\\']);
            (clean.replace('\\', "/"), clean.replace('/', "\\"))
        }
        None => (String::new(), String::new()),
    };
    let is_proj = filter_proj.is_some();

    // 1. Tool Breakdown
    let tool_sql = if is_proj {
        "SELECT tool_name, operation, COUNT(*) AS cnt,
                COALESCE(SUM(tokens_original), 0) AS tok_orig,
                COALESCE(SUM(tokens_optimized), 0) AS tok_opt
         FROM tool_calls
         WHERE started_at >= ?1 AND tool_name != 'mcp_tool' AND (project_path = ?2 COLLATE NOCASE OR project_path = ?3 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?2 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?3 COLLATE NOCASE)
         GROUP BY tool_name, operation ORDER BY cnt DESC LIMIT 50"
    } else {
        "SELECT tool_name, operation, COUNT(*) AS cnt,
                COALESCE(SUM(tokens_original), 0) AS tok_orig,
                COALESCE(SUM(tokens_optimized), 0) AS tok_opt
         FROM tool_calls
         WHERE started_at >= ?1 AND tool_name != 'mcp_tool'
         GROUP BY tool_name, operation ORDER BY cnt DESC LIMIT 50"
    };

    let mut stmt = conn.prepare(tool_sql)?;
    let tool_breakdown: Vec<Value> = if is_proj {
        stmt.query_map(params![cutoff_24h, p1, p2], map_tool_row)?
    } else {
        stmt.query_map([cutoff_24h], map_tool_row)?
    }
    .filter_map(|r| r.ok())
    .collect();

    // 2. Top Skills & Recent Skills
    let mut stmt = conn.prepare("SELECT skill_id, COUNT(*) AS cnt FROM skill_loads WHERE loaded_at >= ? GROUP BY skill_id ORDER BY cnt DESC LIMIT 100")?;
    let mut top_skills_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    let rows = stmt.query_map([cutoff_24h], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for (raw_id, cnt) in rows.flatten() {
        if let Some(clean_id) = normalize_skill_name(&raw_id) {
            *top_skills_map.entry(clean_id).or_default() += cnt;
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
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for item in rows.flatten() {
        if let Some(clean_id) = normalize_skill_name(&item.0) {
            recent_skill_calls.push(json!({ "skill_id": clean_id, "loaded_at": item.1 }));
            if recent_skill_calls.len() >= 50 {
                break;
            }
        }
    }

    // 3. Recent Actions
    let recent_sql = if is_proj {
        "SELECT tool_name, operation, started_at, duration_ms, tokens_original, tokens_optimized, error_message, target
         FROM tool_calls
         WHERE tool_name != 'mcp_tool' AND (project_path = ?1 COLLATE NOCASE OR project_path = ?2 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?1 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?2 COLLATE NOCASE)
         ORDER BY started_at DESC LIMIT 50"
    } else {
        "SELECT tool_name, operation, started_at, duration_ms, tokens_original, tokens_optimized, error_message, target
         FROM tool_calls
         WHERE tool_name != 'mcp_tool'
         ORDER BY started_at DESC LIMIT 50"
    };
    let mut stmt = conn.prepare(recent_sql)?;
    let recent_actions: Vec<Value> = if is_proj {
        stmt.query_map(params![p1, p2], map_recent_action)?
    } else {
        stmt.query_map([], map_recent_action)?
    }
    .filter_map(|r| r.ok())
    .collect();

    // 4. Hourly Savings
    let hourly_savings = query_hourly_savings(&conn, now, is_proj, &p1, &p2);

    // 5. Summaries
    let summaries = query_summaries(&conn, now, cutoff_24h, is_proj, &p1, &p2);

    let mut stmt = conn.prepare("SELECT query_text, queried_at FROM embed_queries WHERE queried_at >= ? ORDER BY queried_at DESC LIMIT 50")?;
    let embed_recent: Vec<Value> = stmt.query_map([cutoff_24h], |row| {
        Ok(json!({ "query": row.get::<_, String>(0)?, "created_at": row.get::<_, i64>(1)? }))
    })?.filter_map(|r| r.ok()).collect();

    let past_24h = summaries["past_24h"].clone();
    Ok(json!({
        "db_status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "project_path": proj_filter.unwrap_or("all"),
        "server_port": 3000,
        "totals": past_24h,
        "summaries": summaries,
        "tool_breakdown": tool_breakdown,
        "top_skills": top_skills,
        "recent_skill_calls": recent_skill_calls,
        "recent_actions": recent_actions,
        "hourly_savings": hourly_savings,
        "embed_recent": embed_recent
    }))
}

fn map_tool_row(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "tool_name": row.get::<_, String>(0)?,
        "operation": row.get::<_, Option<String>>(1)?,
        "cnt": row.get::<_, i64>(2)?,
        "tok_orig": row.get::<_, i64>(3)?,
        "tok_opt": row.get::<_, i64>(4)?,
    }))
}

fn map_recent_action(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "tool_name": row.get::<_, String>(0)?,
        "operation": row.get::<_, Option<String>>(1)?,
        "started_at": row.get::<_, i64>(2)?,
        "duration_ms": row.get::<_, Option<i64>>(3)?.unwrap_or(0),
        "tokens_original": row.get::<_, Option<i64>>(4)?.unwrap_or(0),
        "tokens_optimized": row.get::<_, Option<i64>>(5)?.unwrap_or(0),
        "error_message": row.get::<_, Option<String>>(6)?,
        "target": row.get::<_, Option<String>>(7)?,
    }))
}

fn query_hourly_savings(conn: &Connection, now: i64, is_proj: bool, p1: &str, p2: &str) -> Vec<Value> {
    let current_hour = now / 3600;
    let mut hourly_savings = Vec::with_capacity(24);
    for i in 0..24 {
        let bucket = (current_hour - 23) + i;
        let bucket_start = bucket * 3600;
        let bucket_end = bucket_start + 3600;

        let (orig, opt): (i64, i64) = if is_proj {
            conn.query_row(
                "SELECT COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0)
                 FROM tool_calls WHERE started_at >= ?1 AND started_at < ?2 AND (project_path = ?3 COLLATE NOCASE OR project_path = ?4 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?3 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?4 COLLATE NOCASE)",
                params![bucket_start, bucket_end, p1, p2],
                |r| Ok((r.get(0)?, r.get(1)?)),
            ).unwrap_or((0, 0))
        } else {
            conn.query_row(
                "SELECT COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0)
                 FROM tool_calls WHERE started_at >= ?1 AND started_at < ?2",
                params![bucket_start, bucket_end],
                |r| Ok((r.get(0)?, r.get(1)?)),
            ).unwrap_or((0, 0))
        };

        hourly_savings.push(json!({
            "hour": format!("{:02}:00", bucket % 24),
            "date": format!("{:02}:00", bucket % 24),
            "original": orig,
            "optimized": opt,
            "saved": orig - opt
        }));
    }
    hourly_savings
}

fn query_timeframe_summary(conn: &Connection, cutoff: i64, is_proj: bool, p1: &str, p2: &str) -> Value {
    let (calls, orig, opt): (i64, i64, i64) = if is_proj {
        conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0)
             FROM tool_calls WHERE started_at >= ?1 AND tool_name != 'mcp_tool' AND (project_path = ?2 COLLATE NOCASE OR project_path = ?3 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?2 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?3 COLLATE NOCASE)",
            params![cutoff, p1, p2],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap_or((0, 0, 0))
    } else {
        conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0)
             FROM tool_calls WHERE started_at >= ?1 AND tool_name != 'mcp_tool'",
            params![cutoff],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap_or((0, 0, 0))
    };

    let skills: i64 = conn.query_row(
        "SELECT COUNT(*) FROM skill_loads WHERE loaded_at >= ?1",
        params![cutoff],
        |r| r.get(0),
    ).unwrap_or(0);

    let embeds: i64 = conn.query_row(
        "SELECT COUNT(*) FROM embed_queries WHERE queried_at >= ?1",
        params![cutoff],
        |r| r.get(0),
    ).unwrap_or(0);

    let saved = orig - opt;
    let pct = if orig > 0 { ((saved as f64 / orig as f64) * 1000.0).round() / 10.0 } else { 0.0 };

    json!({
        "tool_calls": calls,
        "skills_loaded": skills,
        "embed_queries": embeds,
        "llm_queries": 0,
        "tokens_original": orig,
        "tokens_optimized": opt,
        "token_savings": saved,
        "savings_pct": pct
    })
}

fn query_summaries(conn: &Connection, now: i64, cutoff_24h: i64, is_proj: bool, p1: &str, p2: &str) -> Value {
    let cutoff_7d = now.saturating_sub(7 * 86400);
    let cutoff_30d = now.saturating_sub(30 * 86400);
    json!({
        "past_24h": query_timeframe_summary(conn, cutoff_24h, is_proj, p1, p2),
        "last_7d": query_timeframe_summary(conn, cutoff_7d, is_proj, p1, p2),
        "last_30d": query_timeframe_summary(conn, cutoff_30d, is_proj, p1, p2),
        "lifetime": query_timeframe_summary(conn, 0, is_proj, p1, p2),
    })
}

fn normalize_skill_name(raw: &str) -> Option<String> {
    let mut s = raw.trim();
    if let Some(idx) = s.find(" (") {
        s = &s[..idx];
    }
    let s = s.trim_matches('`').trim_matches('*').trim_matches('\'').trim_matches('"').trim();
    if s.is_empty() {
        return None;
    }
    if s.contains('/') || s.contains('\\') {
        let p = std::path::Path::new(s);
        if let Some(name) = p.file_name().and_then(|f| f.to_str()) {
            if name.eq_ignore_ascii_case("skill.md") {
                if let Some(parent) = p.parent().and_then(|p| p.file_name()).and_then(|f| f.to_str()) {
                    return Some(parent.to_string());
                }
            } else if name.ends_with(".md") {
                return Some(name.trim_end_matches(".md").to_string());
            }
        }
    }
    Some(s.to_string())
}

fn ensure_tables(conn: &Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS tool_calls (id INTEGER PRIMARY KEY AUTOINCREMENT, tool_name TEXT NOT NULL, operation TEXT, started_at INTEGER NOT NULL, duration_ms INTEGER, tokens_original INTEGER DEFAULT 0, tokens_optimized INTEGER DEFAULT 0, error_message TEXT, project_path TEXT, target TEXT)", []);
    let _ = conn.execute("ALTER TABLE tool_calls ADD COLUMN target TEXT", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS skill_loads (id INTEGER PRIMARY KEY AUTOINCREMENT, skill_id TEXT NOT NULL, loaded_at INTEGER NOT NULL)", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS embed_queries (id INTEGER PRIMARY KEY AUTOINCREMENT, query_text TEXT NOT NULL, queried_at INTEGER NOT NULL)", []);
    let _ = conn.execute("DELETE FROM tool_calls WHERE tool_name = 'mcp_tool'", []);
}
