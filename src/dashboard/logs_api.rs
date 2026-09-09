// Dashboard REST API Handler for MCP Logs and System Diagnostics
use serde_json::json;
use std::collections::HashMap;
use tiny_http::{Request, Response, StatusCode};

pub fn handle_api_logs(request: Request) {
    let url = request.url().to_string();
    let method = request.method().as_str().to_string();

    let (path, query_str) = match url.split_once('?') {
        Some((p, q)) => (p, q),
        None => (url.as_str(), ""),
    };

    let params = parse_query_params(query_str);
    let db_path = crate::mcp::db::get_db_path();

    if method == "POST" && (path.ends_with("/clear") || params.get("action").map(|s| s.as_str()) == Some("clear")) {
        let level_filter = params.get("level").map(|s| s.as_str());
        match crate::mcp::mcp_logger::clear_mcp_logs(&db_path, level_filter) {
            Ok(deleted) => {
                let body = json!({
                    "status": "ok",
                    "cleared_count": deleted,
                    "message": format!("Successfully cleared {} log entries", deleted)
                });
                json_response(request, 200, &body);
            }
            Err(e) => {
                let err = json!({ "status": "error", "message": e.to_string() });
                json_response(request, 500, &err);
            }
        }
        return;
    }

    if method == "GET" {
        let level = params.get("level").map(|s| s.as_str());
        let search = params.get("search").map(|s| s.as_str());
        let limit = params
            .get("limit")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(50)
            .clamp(1, 200);
        let offset = params
            .get("offset")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        match crate::mcp::mcp_logger::query_mcp_logs(&db_path, level, search, limit, offset) {
            Ok((logs, stats)) => {
                let body = json!({
                    "status": "ok",
                    "logs": logs,
                    "stats": stats,
                    "limit": limit,
                    "offset": offset,
                    "retention_policy": {
                        "crash_days": 30,
                        "error_days": 14,
                        "warn_days": 7,
                        "max_entries": crate::mcp::mcp_logger::MAX_SQLITE_LOGS
                    }
                });
                json_response(request, 200, &body);
            }
            Err(e) => {
                let err = json!({ "status": "error", "message": e.to_string() });
                json_response(request, 500, &err);
            }
        }
        return;
    }

    let err = json!({ "status": "error", "message": "Method not allowed" });
    json_response(request, 405, &err);
}

fn parse_query_params(query_str: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for pair in query_str.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((k, v)) = pair.split_once('=') {
            let decoded_val = urlencoding_decode(v);
            params.insert(k.to_string(), decoded_val);
        } else {
            params.insert(pair.to_string(), "".to_string());
        }
    }
    params
}

fn urlencoding_decode(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'+' {
            res.push(' ');
        } else if b == b'%' {
            if let (Some(h1), Some(h2)) = (bytes.next(), bytes.next()) {
                if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&[h1, h2]).unwrap_or(""), 16) {
                    res.push(val as char);
                    continue;
                }
            }
            res.push('%');
        } else {
            res.push(b as char);
        }
    }
    res
}

fn json_response(request: Request, status_code: u16, data: &serde_json::Value) {
    let body = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    let header_ct = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap();
    let header_cors = tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();
    let response = Response::from_string(body)
        .with_header(header_ct)
        .with_header(header_cors)
        .with_status_code(StatusCode(status_code));
    let _ = request.respond(response);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_params() {
        let p = parse_query_params("level=crash&search=test+panic&limit=25");
        assert_eq!(p.get("level").map(|s| s.as_str()), Some("crash"));
        assert_eq!(p.get("search").map(|s| s.as_str()), Some("test panic"));
        assert_eq!(p.get("limit").map(|s| s.as_str()), Some("25"));
    }

    #[test]
    fn test_live_db_query() {
        let db_path = crate::mcp::db::get_db_path();
        let res = crate::mcp::mcp_logger::query_mcp_logs(&db_path, None, None, 100, 0);
        assert!(res.is_ok(), "query_mcp_logs failed: {:?}", res.err());
    }
}
