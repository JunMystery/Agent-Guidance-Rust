use super::{DashboardAssets, StatsCache, graph, logs_api, projects, stats::handle_api_stats};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Response, StatusCode};

pub(crate) fn handle_dashboard_request(
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
        "/favicon.ico" | "/docs/images/logo.ico" => {
            let ico_bytes = include_bytes!("../../docs/images/logo.ico");
            let header = Header::from_bytes(&b"Content-Type"[..], &b"image/x-icon"[..]).unwrap();
            let _ = request.respond(Response::from_data(ico_bytes.as_slice()).with_header(header));
        }
        "/favicon.png" | "/logo.png" | "/docs/images/logo.png" => {
            let png_bytes = include_bytes!("../../docs/images/logo.png");
            let header = Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap();
            let _ = request.respond(Response::from_data(png_bytes.as_slice()).with_header(header));
        }
        "/api/stats" => handle_api_stats(request, project_path, cache),
        "/api/engine/refresh" => handle_api_engine_refresh(request, cache),
        "/api/projects" => {
            let db_path = dirs::home_dir()
                .map(|h| h.join(".agent-guidance").join("usage.db"))
                .unwrap_or_else(|| std::path::PathBuf::from("usage.db"));
            projects::handle_api_projects(request, &db_path);
        }
        "/api/projects/prune" => {
            let db_path = dirs::home_dir()
                .map(|h| h.join(".agent-guidance").join("usage.db"))
                .unwrap_or_else(|| std::path::PathBuf::from("usage.db"));
            projects::handle_api_prune(request, &db_path);
        }
        "/api/graph" => graph::handle_api_graph(request, project_path),
        "/api/cleanup" => handle_api_cleanup(request),
        "/api/logs" | "/api/logs/clear" => logs_api::handle_api_logs(request),
        "/health" => {
            let db_bytes = crate::mcp::db::get_db_size_bytes();
            let json_data = json!({
                "status": "ok",
                "server": "agent-guidance-dashboard",
                "version": env!("CARGO_PKG_VERSION"),
                "model_loaded": true,
                "engine": "rust-candle",
                "backend": "candle-bert",
                "db_size_bytes": db_bytes,
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

pub(crate) fn serve_asset(request: tiny_http::Request, name: &str, mime_type: &str) {
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

pub(crate) fn json_response(request: tiny_http::Request, status_code: u16, data: &serde_json::Value) {
    let body = serde_json::to_string(data).unwrap_or_default();
    let header_ct = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    let origin_str = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Origin"))
        .map(|h| h.value.as_str())
        .filter(|o| o.starts_with("http://127.0.0.1") || o.starts_with("http://localhost"))
        .unwrap_or("http://127.0.0.1:11997");
    let header_cors = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], origin_str.as_bytes()).unwrap();
    let response = Response::from_string(body)
        .with_header(header_ct)
        .with_header(header_cors)
        .with_status_code(StatusCode(status_code));
    let _ = request.respond(response);
}

fn handle_api_cleanup(request: tiny_http::Request) {
    let db_path = dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("usage.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("usage.db"));

    let conn = match rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            json_response(request, 500, &json!({"error": e.to_string()}));
            return;
        }
    };

    match crate::mcp::db::run_auto_cleanup(&conn, crate::mcp::db::cleanup::DEFAULT_RETENTION_DAYS) {
        Ok(summary) => {
            let db_bytes = crate::mcp::db::get_db_size_bytes();
            json_response(
                request,
                200,
                &json!({
                    "success": true,
                    "summary": summary,
                    "db_size_bytes": db_bytes,
                    "db_size_mb": format!("{:.2} MB", db_bytes as f64 / (1024.0 * 1024.0))
                }),
            );
        }
        Err(e) => json_response(request, 500, &json!({"error": e.to_string()})),
    }
}

fn handle_api_engine_refresh(request: tiny_http::Request, cache: &Arc<Mutex<StatsCache>>) {
    crate::ml::embeddings::cache::clear_passage_cache();
    crate::ml::embeddings::cache::warmup_cache();
    if let Ok(mut guard) = cache.lock() {
        guard.data = None;
    }
    let db_bytes = crate::mcp::db::get_db_size_bytes();
    json_response(
        request,
        200,
        &json!({
            "success": true,
            "message": "Neural vector embeddings cache refreshed and model checkpoints reloaded",
            "status": "ok",
            "engine": "rust-candle",
            "backend": "candle-bert",
            "model_loaded": true,
            "db_size_bytes": db_bytes,
            "clients": crate::daemon::active_clients_count()
        }),
    );
}
