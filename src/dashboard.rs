use anyhow::Result;
use rust_embed::Embed;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc::sync_channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Response, Server, StatusCode};

#[derive(Embed)]
#[folder = "src/dashboard_src/"]
pub struct DashboardAssets;

const STATS_CACHE_TTL: Duration = Duration::from_secs(2);
const DASHBOARD_WORKERS: usize = 4;
const DASHBOARD_QUEUE: usize = 32;

#[derive(Default)]
struct StatsCache {
    generated_at: Option<Instant>,
    data: Option<serde_json::Value>,
}

pub fn run_dashboard_server(port: u16, project_path: Option<String>) -> Result<()> {
    let proj_dir = project_path.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    });

    let addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to bind server to {}: {}", addr, e))?;
    println!("✓ Usage Dashboard server listening on http://{}", addr);
    let stats_cache = Arc::new(Mutex::new(StatsCache::default()));

    let (sender, receiver) = sync_channel(DASHBOARD_QUEUE);
    let receiver = Arc::new(Mutex::new(receiver));
    for _ in 0..DASHBOARD_WORKERS {
        let receiver = receiver.clone();
        let project_path = proj_dir.clone();
        let cache = stats_cache.clone();
        std::thread::spawn(move || {
            loop {
                let request = match receiver.lock().ok().and_then(|queue| queue.recv().ok()) {
                    Some(request) => request,
                    None => break,
                };
                handle_dashboard_request(request, &project_path, &cache);
            }
        });
    }

    for request in server.incoming_requests() {
        if sender.send(request).is_err() {
            break;
        }
    }

    Ok(())
}

fn handle_dashboard_request(
    request: tiny_http::Request,
    project_path: &str,
    cache: &Arc<Mutex<StatsCache>>,
) {
    let url = request
        .url()
        .split('?')
        .next()
        .unwrap_or("/")
        .trim_end_matches('/');
    let path = if url.is_empty() { "/" } else { url };

    match path {
        "/" | "/index.html" => serve_asset(request, "index.html", "text/html; charset=utf-8"),
        "/dashboard.css" => serve_asset(request, "dashboard.css", "text/css; charset=utf-8"),
        "/api/stats" => handle_api_stats(request, project_path, cache),
        "/health" => {
            let json_data = json!({
                "status": "ok",
                "server": "agent-guidance-dashboard",
                "version": env!("CARGO_PKG_VERSION"),
                "model_loaded": true,
                "engine": "rust-candle",
                "backend": "candle-bert",
                "clients": crate::daemon::active_clients_count()
            });
            json_response(request, 200, &json_data);
        }
        _ if path.starts_with("/js/") => {
            let rel = path.trim_start_matches("/js/");
            let asset_path = format!("js/{}", rel);
            serve_asset(
                request,
                &asset_path,
                "application/javascript; charset=utf-8",
            );
        }
        _ => json_response(request, 404, &json!({"error": "Not found"})),
    }
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
        json_response(
            request,
            404,
            &json!({"error": format!("Asset '{}' not found", name)}),
        );
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

fn handle_api_stats(request: tiny_http::Request, proj_dir: &str, cache: &Arc<Mutex<StatsCache>>) {
    if let Ok(guard) = cache.lock() {
        if let (Some(generated_at), Some(data)) = (guard.generated_at, guard.data.as_ref()) {
            if generated_at.elapsed() < STATS_CACHE_TTL {
                json_response(request, 200, data);
                return;
            }
        }
    }

    let db_path = dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("usage.db"))
        .unwrap_or_else(|| PathBuf::from("usage.db"));

    if !db_path.exists() {
        json_response(
            request,
            200,
            &json!({
                "success": false,
                "error": "NO_USAGE_DATA",
                "db_status": "missing",
                "message": format!("No usage.db found at {:?}", db_path)
            }),
        );
        return;
    }

    match query_usage_stats(&db_path, proj_dir) {
        Ok(data) => {
            if let Ok(mut guard) = cache.lock() {
                guard.generated_at = Some(Instant::now());
                guard.data = Some(data.clone());
            }
            json_response(request, 200, &data);
        }
        Err(e) => json_response(request, 500, &json!({"error": e.to_string()})),
    }
}

fn get_iso_date(now_secs: i64) -> String {
    let days = now_secs / 86400;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", year, m, d)
}

fn query_usage_stats(db_path: &PathBuf, proj_dir: &str) -> Result<serde_json::Value> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    let _ = conn.execute("CREATE TABLE IF NOT EXISTS tool_calls (id INTEGER PRIMARY KEY AUTOINCREMENT, tool_name TEXT NOT NULL, operation TEXT, started_at INTEGER NOT NULL, duration_ms INTEGER, tokens_original INTEGER DEFAULT 0, tokens_optimized INTEGER DEFAULT 0, error_message TEXT)", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS skill_loads (id INTEGER PRIMARY KEY AUTOINCREMENT, skill_id TEXT NOT NULL, loaded_at INTEGER NOT NULL)", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS embed_queries (id INTEGER PRIMARY KEY AUTOINCREMENT, query_text TEXT NOT NULL, queried_at INTEGER NOT NULL)", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS llm_queries (id INTEGER PRIMARY KEY AUTOINCREMENT, query_text TEXT NOT NULL, queried_at INTEGER NOT NULL)", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS daily_summaries (day TEXT PRIMARY KEY, tool_calls INTEGER DEFAULT 0, skills_loaded INTEGER DEFAULT 0, embed_queries INTEGER DEFAULT 0, llm_queries INTEGER DEFAULT 0, tokens_original INTEGER DEFAULT 0, tokens_optimized INTEGER DEFAULT 0)", []);

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let cutoff_24h = now - 86400;

    let mut stmt = conn.prepare(
        "SELECT tool_name, operation, COUNT(*) AS cnt,
                COALESCE(SUM(tokens_original), 0) AS tok_orig,
                COALESCE(SUM(tokens_optimized), 0) AS tok_opt
         FROM tool_calls WHERE started_at >= ?
         GROUP BY tool_name, operation ORDER BY cnt DESC LIMIT 50",
    )?;
    let tool_breakdown: Vec<serde_json::Value> = stmt
        .query_map([cutoff_24h], |row| {
            Ok(json!({
                "tool_name": row.get::<_, String>(0)?,
                "operation": row.get::<_, Option<String>>(1)?,
                "cnt": row.get::<_, i64>(2)?,
                "tok_orig": row.get::<_, i64>(3)?,
                "tok_opt": row.get::<_, i64>(4)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT skill_id, COUNT(*) AS cnt FROM skill_loads WHERE loaded_at >= ? GROUP BY skill_id ORDER BY cnt DESC LIMIT 50"
    )?;
    let top_skills: Vec<serde_json::Value> = stmt
        .query_map([cutoff_24h], |row| {
            Ok(json!({
                "skill_id": row.get::<_, String>(0)?,
                "cnt": row.get::<_, i64>(1)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn
        .prepare("SELECT skill_id, loaded_at FROM skill_loads ORDER BY loaded_at DESC LIMIT 10")?;
    let recent_skill_calls: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "skill_id": row.get::<_, String>(0)?,
                "loaded_at": row.get::<_, i64>(1)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT tool_name, operation, started_at, duration_ms, tokens_original, tokens_optimized, error_message
         FROM tool_calls WHERE started_at >= ? ORDER BY started_at DESC LIMIT 50"
    )?;
    let recent_actions: Vec<serde_json::Value> = stmt
        .query_map([cutoff_24h], |row| {
            Ok(json!({
                "tool_name": row.get::<_, String>(0)?,
                "operation": row.get::<_, Option<String>>(1)?,
                "started_at": row.get::<_, i64>(2)?,
                "duration_ms": row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                "tokens_original": row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                "tokens_optimized": row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                "error_message": row.get::<_, Option<String>>(6)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let current_hour = now / 3600;
    let mut hourly_savings = Vec::new();
    for i in 0..24 {
        let bucket = (current_hour - 23) + i;
        let bucket_start = bucket * 3600;
        let bucket_end = bucket_start + 3600;

        let (orig, opt): (i64, i64) = conn
            .query_row(
                "SELECT COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0)
             FROM tool_calls WHERE started_at >= ? AND started_at < ?",
                [bucket_start, bucket_end],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0, 0));

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

    // Timeframe Summaries: Past 24H, Last 7 Days, Last 30 Days, Lifetime
    let day_7d_ago = get_iso_date(now - 7 * 86400);
    let day_30d_ago = get_iso_date(now - 30 * 86400);

    let get_summary = |query: &str, params: &[&dyn rusqlite::ToSql]| -> serde_json::Value {
        let res: Result<(i64, i64, i64, i64, i64, i64), _> = conn.query_row(query, params, |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        });
        let (calls, skills, embeds, llms, orig, opt) = res.unwrap_or((0, 0, 0, 0, 0, 0));
        let saved = orig - opt;
        let pct = if orig > 0 {
            ((saved as f64 / orig as f64) * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };
        json!({
            "tool_calls": calls,
            "skills_loaded": skills,
            "embed_queries": embeds,
            "llm_queries": llms,
            "tokens_original": orig,
            "tokens_optimized": opt,
            "token_savings": saved,
            "savings_pct": pct
        })
    };

    let p24_calls: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tool_calls WHERE started_at >= ?",
            [cutoff_24h],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let p24_skills: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM skill_loads WHERE loaded_at >= ?",
            [cutoff_24h],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let p24_embeds: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM embed_queries WHERE queried_at >= ?",
            [cutoff_24h],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let p24_llm: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM llm_queries WHERE queried_at >= ?",
            [cutoff_24h],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let (p24_orig, p24_opt): (i64, i64) = conn.query_row("SELECT COALESCE(SUM(tokens_original), 0), COALESCE(SUM(tokens_optimized), 0) FROM tool_calls WHERE started_at >= ?", [cutoff_24h], |r| Ok((r.get(0)?, r.get(1)?))).unwrap_or((0, 0));
    let p24_saved = p24_orig - p24_opt;
    let p24_pct = if p24_orig > 0 {
        ((p24_saved as f64 / p24_orig as f64) * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    let past_24h_summary = json!({
        "tool_calls": p24_calls,
        "skills_loaded": p24_skills,
        "embed_queries": p24_embeds,
        "llm_queries": p24_llm,
        "tokens_original": p24_orig,
        "tokens_optimized": p24_opt,
        "token_savings": p24_saved,
        "savings_pct": p24_pct
    });

    let sql_sum = "SELECT COALESCE(SUM(tool_calls),0), COALESCE(SUM(skills_loaded),0), COALESCE(SUM(embed_queries),0), COALESCE(SUM(llm_queries),0), COALESCE(SUM(tokens_original),0), COALESCE(SUM(tokens_optimized),0) FROM daily_summaries";

    let last_7d_summary = get_summary(&format!("{} WHERE day >= ?", sql_sum), &[&day_7d_ago]);
    let last_30d_summary = get_summary(&format!("{} WHERE day >= ?", sql_sum), &[&day_30d_ago]);
    let lifetime_summary = get_summary(sql_sum, &[]);

    let mut stmt = conn.prepare(
        "SELECT query_text, queried_at FROM embed_queries WHERE queried_at >= ? ORDER BY queried_at DESC LIMIT 50"
    )?;
    let embed_recent: Vec<serde_json::Value> = stmt
        .query_map([cutoff_24h], |row| {
            Ok(json!({
                "query": row.get::<_, String>(0)?,
                "created_at": row.get::<_, i64>(1)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!({
        "db_status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "project_path": proj_dir,
        "server_port": 3000,
        "totals": past_24h_summary,
        "summaries": {
            "past_24h": past_24h_summary,
            "last_7d": last_7d_summary,
            "last_30d": last_30d_summary,
            "lifetime": lifetime_summary
        },
        "tool_breakdown": tool_breakdown,
        "top_skills": top_skills,
        "recent_skill_calls": recent_skill_calls,
        "recent_actions": recent_actions,
        "hourly_savings": hourly_savings,
        "embed_recent": embed_recent
    }))
}
