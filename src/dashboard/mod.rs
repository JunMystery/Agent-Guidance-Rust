use anyhow::Result;
use rust_embed::Embed;
use serde_json::json;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, mpsc::sync_channel};
use std::time::{Duration, Instant};
use tiny_http::{Header, Response, Server, StatusCode};
use tracing::info;

pub mod graph;
pub mod logs_api;
pub mod projects;
pub mod stats;
pub mod stats_query;
use stats::handle_api_stats;

#[derive(Embed)]
#[folder = "src/dashboard_src/"]
pub struct DashboardAssets;

pub const DEFAULT_DASHBOARD_PORT: u16 = 11997;
pub(crate) static DASHBOARD_PORT: AtomicU16 = AtomicU16::new(DEFAULT_DASHBOARD_PORT);

pub fn get_dashboard_port() -> u16 {
    DASHBOARD_PORT.load(Ordering::Relaxed)
}

pub fn spawn_dashboard_background(port: u16, project_path: Option<String>) {
    DASHBOARD_PORT.store(port, Ordering::SeqCst);
    let _ = std::thread::Builder::new()
        .name("dashboard-background".to_string())
        .spawn(move || {
            if let Err(e) = run_dashboard_server(port, project_path) {
                tracing::warn!("Dashboard background server could not bind or stopped on port {}: {}", port, e);
            }
        });
}

pub(crate) const STATS_CACHE_TTL: Duration = Duration::from_secs(2);
const DASHBOARD_WORKERS: usize = 4;
const DASHBOARD_QUEUE: usize = 32;

#[derive(Default)]
pub(crate) struct StatsCache {
    pub(crate) generated_at: Option<Instant>,
    pub(crate) data: Option<serde_json::Value>,
    pub(crate) project_key: Option<String>,
}

pub fn run_dashboard_server(port: u16, project_path: Option<String>) -> Result<()> {
    DASHBOARD_PORT.store(port, Ordering::SeqCst);
    let proj_dir = project_path.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    });

    let addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to bind server to {}: {}", addr, e))?;
    info!("Usage Dashboard server listening on http://{}", addr);
    if proj_dir != "." && proj_dir != "all" {
        if std::path::Path::new(&proj_dir).exists() {
            info!("Focused on project: {}", proj_dir);
        } else {
            info!("Project '{}' not found on disk. Displaying archived historical analytics.", proj_dir);
        }
    } else {
        info!("Viewing all tracked projects. Use project dropdown in UI to filter.");
    }
    let _ = crate::mcp::db::init_db();
    let _ = projects::prune_missing_projects(&crate::mcp::db::get_db_path());
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
        "/favicon.ico" | "/favicon.png" | "/logo.png" => {
            let png_bytes = include_bytes!("../../docs/images/logo.png");
            let header = Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap();
            let _ = request.respond(Response::from_data(png_bytes.as_slice()).with_header(header));
        }
        "/api/stats" => handle_api_stats(request, project_path, cache),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_dashboard_port_is_11997() {
        assert_eq!(DEFAULT_DASHBOARD_PORT, 11997);
        assert_eq!(get_dashboard_port(), 11997);
    }

    #[test]
    fn test_custom_dashboard_port_mutation() {
        DASHBOARD_PORT.store(12345, Ordering::SeqCst);
        assert_eq!(get_dashboard_port(), 12345);
        DASHBOARD_PORT.store(DEFAULT_DASHBOARD_PORT, Ordering::SeqCst);
        assert_eq!(get_dashboard_port(), 11997);
    }
}


