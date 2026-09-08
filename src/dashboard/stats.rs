use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::{StatsCache, STATS_CACHE_TTL, json_response};
use super::stats_query::query_usage_stats;

pub(crate) fn handle_api_stats(request: tiny_http::Request, default_proj: &str, cache: &Arc<Mutex<StatsCache>>) {
    let url = request.url();
    let query_project = extract_query_param(url, "project")
        .unwrap_or_else(|| {
            if default_proj != "." && default_proj != "all" {
                default_proj.to_string()
            } else {
                "all".to_string()
            }
        });

    if let Ok(guard) = cache.lock() {
        if let (Some(gen_at), Some(data), Some(cached_key)) = (guard.generated_at, guard.data.as_ref(), guard.project_key.as_ref()) {
            if cached_key == &query_project && gen_at.elapsed() < STATS_CACHE_TTL {
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

    match query_usage_stats(&db_path, Some(&query_project)) {
        Ok(data) => {
            if let Ok(mut guard) = cache.lock() {
                guard.project_key = Some(query_project);
                guard.generated_at = Some(Instant::now());
                guard.data = Some(data.clone());
            }
            json_response(request, 200, &data);
        }
        Err(e) => json_response(request, 500, &json!({"error": e.to_string()})),
    }
}

fn extract_query_param(url: &str, param: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?;
        if key == param {
            let val = parts.next().unwrap_or("");
            return Some(url_decode(val));
        }
    }
    None
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h1 = bytes.next().unwrap_or(b'0') as char;
            let h2 = bytes.next().unwrap_or(b'0') as char;
            if let Ok(hex) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                result.push(hex as char);
            }
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}
