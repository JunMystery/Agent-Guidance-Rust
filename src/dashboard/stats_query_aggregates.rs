use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub fn query_hourly_savings(conn: &Connection, now: i64, is_proj: bool, p1: &str, p2: &str) -> Vec<Value> {
    let mut hourly_map = std::collections::BTreeMap::new();
    let start_of_period = now - (23 * 3600);
    let start_hour = (start_of_period / 3600) * 3600;

    for i in 0..24 {
        let h = start_hour + (i * 3600);
        hourly_map.insert(h, (0i64, 0i64, 0i64));
    }

    let h_sql = if is_proj {
        "SELECT (started_at / 3600) * 3600 as hour, COUNT(*), COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0)
         FROM tool_calls WHERE started_at >= ?1 AND tool_name != 'mcp_tool' AND (project_path = ?2 COLLATE NOCASE OR project_path = ?3 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?2 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?3 COLLATE NOCASE) GROUP BY hour"
    } else {
        "SELECT (started_at / 3600) * 3600 as hour, COUNT(*), COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0)
         FROM tool_calls WHERE started_at >= ?1 AND tool_name != 'mcp_tool' GROUP BY hour"
    };

    if let Ok(mut stmt) = conn.prepare(h_sql) {
        let mapper = |row: &rusqlite::Row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?));
        let rows = if is_proj { stmt.query_map(params![start_hour, p1, p2], mapper) }
                   else { stmt.query_map([start_hour], mapper) };
        if let Ok(iter) = rows {
            for (h, cnt, orig, opt) in iter.flatten() {
                hourly_map.insert(h, (cnt, orig, opt));
            }
        }
    }

    hourly_map
        .into_iter()
        .map(|(h, (cnt, orig, opt))| {
            let saved = orig - opt;
            json!({
                "hour": h,
                "calls": cnt,
                "tokens_original": orig,
                "tokens_optimized": opt,
                "tokens_saved": saved
            })
        })
        .collect()
}

pub fn query_phase_and_gov(conn: &Connection, cutoff: i64, is_proj: bool, p1: &str, p2: &str) -> (Value, Value) {
    let sql = if is_proj {
        "SELECT tool_name, LOWER(COALESCE(operation, '')), COUNT(*) FROM tool_calls WHERE started_at >= ?1 AND (tool_name = 'task_pipeline' OR tool_name = 'workflow_gate' OR (tool_name = 'guidance' AND operation = 'verify')) AND (project_path = ?2 COLLATE NOCASE OR project_path = ?3 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?2 COLLATE NOCASE OR rtrim(project_path, '/\\') = ?3 COLLATE NOCASE) GROUP BY tool_name, LOWER(COALESCE(operation, ''))"
    } else {
        "SELECT tool_name, LOWER(COALESCE(operation, '')), COUNT(*) FROM tool_calls WHERE started_at >= ?1 AND (tool_name = 'task_pipeline' OR tool_name = 'workflow_gate' OR (tool_name = 'guidance' AND operation = 'verify')) GROUP BY tool_name, LOWER(COALESCE(operation, ''))"
    };
    let mut phases = json!({ "plan": 0, "build": 0, "test": 0, "fix": 0, "review": 0, "refactor": 0 });
    let mut gov = json!({ "edits_authorized": 0, "plans_approved": 0, "verifications_passed": 0, "stages_transitioned": 0 });
    if let Ok(mut stmt) = conn.prepare(sql) {
        let mapper = |r: &rusqlite::Row| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?));
        let rows = if is_proj { stmt.query_map(params![cutoff, p1, p2], mapper) }
                   else { stmt.query_map([cutoff], mapper) };
        if let Ok(iter) = rows {
            for (t, op, cnt) in iter.flatten() {
                if t == "task_pipeline" {
                    if let Some(v) = phases.get_mut(&op) { *v = json!(v.as_i64().unwrap_or(0) + cnt); }
                } else if op == "authorize_edit" {
                    gov["edits_authorized"] = json!(gov["edits_authorized"].as_i64().unwrap_or(0) + cnt);
                } else if op == "approve_plan" || op == "approve" {
                    gov["plans_approved"] = json!(gov["plans_approved"].as_i64().unwrap_or(0) + cnt);
                } else if op == "verify" || op == "pass_verification" {
                    gov["verifications_passed"] = json!(gov["verifications_passed"].as_i64().unwrap_or(0) + cnt);
                } else if op == "set_stage" || op == "advance" {
                    gov["stages_transitioned"] = json!(gov["stages_transitioned"].as_i64().unwrap_or(0) + cnt);
                }
            }
        }
    }
    (phases, gov)
}

pub fn query_timeframe_summary(conn: &Connection, cutoff: i64, is_proj: bool, p1: &str, p2: &str) -> Value {
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

    let skills: i64 = conn.query_row("SELECT COUNT(*) FROM skill_loads WHERE loaded_at >= ?1", [cutoff], |r| r.get(0)).unwrap_or(0);
    let embeds: i64 = conn.query_row("SELECT COUNT(*) FROM embed_queries WHERE queried_at >= ?1", [cutoff], |r| r.get(0)).unwrap_or(0);
    let saved = orig - opt;
    let pct = if orig > 0 { ((saved as f64 / orig as f64) * 1000.0).round() / 10.0 } else { 0.0 };

    json!({
        "tool_calls": calls, "skills_loaded": skills, "embed_queries": embeds, "llm_queries": 0,
        "tokens_original": orig, "tokens_optimized": opt, "token_savings": saved, "savings_pct": pct
    })
}

pub fn query_summaries(conn: &Connection, now: i64, cutoff_24h: i64, is_proj: bool, p1: &str, p2: &str) -> Value {
    json!({
        "past_24h": query_timeframe_summary(conn, cutoff_24h, is_proj, p1, p2),
        "last_7d": query_timeframe_summary(conn, now.saturating_sub(7 * 86400), is_proj, p1, p2),
        "last_30d": query_timeframe_summary(conn, now.saturating_sub(30 * 86400), is_proj, p1, p2),
        "lifetime": query_timeframe_summary(conn, 0, is_proj, p1, p2),
    })
}
