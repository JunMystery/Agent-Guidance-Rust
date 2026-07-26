use anyhow::Result;
use rust_embed::Embed;
use serde_json::json;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Response, Server, StatusCode};

#[derive(Embed)]
#[folder = "src/dashboard_src/"]
pub struct DashboardAssets;

pub fn run_dashboard_server(port: u16, project_path: Option<String>) -> Result<()> {
    let proj_dir = project_path.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    });

    let addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&addr).map_err(|e| anyhow::anyhow!("Failed to bind server to {}: {}", addr, e))?;
    println!("✓ Usage Dashboard server listening on http://{}", addr);

    for request in server.incoming_requests() {
        let url = request.url().split('?').next().unwrap_or("/").trim_end_matches('/');
        let path = if url.is_empty() { "/" } else { url };

        match path {
            "/" | "/index.html" => {
                serve_asset(request, "index.html", "text/html; charset=utf-8");
            },
            "/dashboard.css" => {
                serve_asset(request, "dashboard.css", "text/css; charset=utf-8");
            },
            "/api/stats" => {
                handle_api_stats(request, &proj_dir);
            },
            "/health" => {
                let json_data = json!({
                    "status": "ok",
                    "server": "agent-guidance-dashboard",
                    "version": env!("CARGO_PKG_VERSION"),
                    "model_loaded": true,
                    "engine": "rust-candle"
                });
                json_response(request, 200, &json_data);
            },
            _ if path.starts_with("/js/") => {
                let rel = path.trim_start_matches("/js/");
                let asset_path = format!("js/{}", rel);
                serve_asset(request, &asset_path, "application/javascript; charset=utf-8");
            },
            _ => {
                json_response(request, 404, &json!({"error": "Not found"}));
            }
        }
    }

    Ok(())
}

fn serve_asset(request: tiny_http::Request, name: &str, mime_type: &str) {
    if let Some(file) = DashboardAssets::get(name) {
        let data = file.data.into_owned();
        let header = Header::from_bytes(&b"Content-Type"[..], mime_type.as_bytes()).unwrap();
        let response = Response::from_data(data)
            .with_header(header)
            .with_status_code(StatusCode(200));
        let _ = request.respond(response);
    } else {
        json_response(request, 404, &json!({"error": format!("Asset '{}' not found", name)}));
    }
}

fn json_response(request: tiny_http::Request, status_code: u16, data: &serde_json::Value) {
    let body = serde_json::to_string(data).unwrap_or_default();
    let header_ct = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    let header_cors = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();
    let response = Response::from_string(body)
        .with_header(header_ct)
        .with_header(header_cors)
        .with_status_code(StatusCode(status_code));
    let _ = request.respond(response);
}

fn handle_api_stats(request: tiny_http::Request, proj_dir: &str) {
    let db_path = dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("usage.db"))
        .unwrap_or_else(|| PathBuf::from("usage.db"));

    if !db_path.exists() {
        json_response(request, 200, &json!({
            "success": false,
            "error": "NO_USAGE_DATA",
            "db_status": "missing",
            "message": format!("No usage.db found at {:?}", db_path)
        }));
        return;
    }

    match query_usage_stats(&db_path, proj_dir) {
        Ok(data) => json_response(request, 200, &data),
        Err(e) => json_response(request, 500, &json!({"error": e.to_string()})),
    }
}

fn query_usage_stats(db_path: &PathBuf, proj_dir: &str) -> Result<serde_json::Value> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let cutoff_24h = now - 86400;

    let mut stmt = conn.prepare(
        "SELECT tool_name, operation, COUNT(*) AS cnt,
                COALESCE(SUM(tokens_original), 0) AS tok_orig,
                COALESCE(SUM(tokens_optimized), 0) AS tok_opt
         FROM tool_calls WHERE started_at >= ?
         GROUP BY tool_name, operation ORDER BY cnt DESC"
    )?;
    let tool_breakdown: Vec<serde_json::Value> = stmt.query_map([cutoff_24h], |row| {
        Ok(json!({
            "tool_name": row.get::<_, String>(0)?,
            "operation": row.get::<_, Option<String>>(1)?,
            "cnt": row.get::<_, i64>(2)?,
            "tok_orig": row.get::<_, i64>(3)?,
            "tok_opt": row.get::<_, i64>(4)?,
        }))
    })?.filter_map(|r| r.ok()).collect();

    let mut stmt = conn.prepare(
        "SELECT skill_id, COUNT(*) AS cnt FROM skill_loads GROUP BY skill_id ORDER BY cnt DESC LIMIT 20"
    )?;
    let top_skills: Vec<serde_json::Value> = stmt.query_map([], |row| {
        Ok(json!({
            "skill_id": row.get::<_, String>(0)?,
            "cnt": row.get::<_, i64>(1)?,
        }))
    })?.filter_map(|r| r.ok()).collect();

    let mut stmt = conn.prepare(
        "SELECT tool_name, operation, started_at, duration_ms, tokens_original, tokens_optimized, error_message
         FROM tool_calls WHERE started_at >= ? ORDER BY started_at DESC LIMIT 20"
    )?;
    let recent_actions: Vec<serde_json::Value> = stmt.query_map([cutoff_24h], |row| {
        Ok(json!({
            "tool_name": row.get::<_, String>(0)?,
            "operation": row.get::<_, Option<String>>(1)?,
            "started_at": row.get::<_, i64>(2)?,
            "duration_ms": row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            "tokens_original": row.get::<_, Option<i64>>(4)?.unwrap_or(0),
            "tokens_optimized": row.get::<_, Option<i64>>(5)?.unwrap_or(0),
            "error_message": row.get::<_, Option<String>>(6)?,
        }))
    })?.filter_map(|r| r.ok()).collect();

    let current_hour = now / 3600;
    let mut hourly_savings = Vec::new();
    for i in 0..24 {
        let bucket = (current_hour - 23) + i;
        let bucket_start = bucket * 3600;
        let bucket_end = bucket_start + 3600;

        let (orig, opt): (i64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0)
             FROM tool_calls WHERE started_at >= ? AND started_at < ?",
            [bucket_start, bucket_end],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap_or((0, 0));

        let saved = orig - opt;
        let date_label = format!("{:02}:00", bucket % 24);

        hourly_savings.push(json!({
            "hour": bucket % 24,
            "date": date_label,
            "original": orig,
            "optimized": opt,
            "saved": saved,
            "is_current": bucket == current_hour
        }));
    }

    let total_calls: i64 = conn.query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0)).unwrap_or(0);
    let total_skills: i64 = conn.query_row("SELECT COUNT(*) FROM skill_loads", [], |r| r.get(0)).unwrap_or(0);
    let total_embeds: i64 = conn.query_row("SELECT COUNT(*) FROM embed_queries", [], |r| r.get(0)).unwrap_or(0);
    let total_llm: i64 = conn.query_row("SELECT COUNT(*) FROM llm_queries", [], |r| r.get(0)).unwrap_or(0);

    let tot_orig: i64 = tool_breakdown.iter().map(|r| r["tok_orig"].as_i64().unwrap_or(0)).sum();
    let tot_opt: i64 = tool_breakdown.iter().map(|r| r["tok_opt"].as_i64().unwrap_or(0)).sum();
    let token_savings = tot_orig - tot_opt;
    let savings_pct = if tot_orig > 0 {
        ((token_savings as f64 / tot_orig as f64) * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    Ok(json!({
        "db_status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "project_path": proj_dir,
        "server_port": 3000,
        "totals": {
            "tool_calls": total_calls,
            "skills_loaded": total_skills,
            "embed_queries": total_embeds,
            "llm_queries": total_llm,
            "tokens_original": tot_orig,
            "tokens_optimized": tot_opt,
            "token_savings": token_savings,
            "savings_pct": savings_pct,
        },
        "tool_breakdown": tool_breakdown,
        "top_skills": top_skills,
        "recent_actions": recent_actions,
        "hourly_savings": hourly_savings,
        "embed_recent": []
    }))
}

