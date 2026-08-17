use anyhow::Result;
use rust_embed::Embed;
use serde_json::json;
use std::sync::{Arc, Mutex, mpsc::sync_channel};
use std::time::{Duration, Instant};
use tiny_http::{Header, Response, Server, StatusCode};

pub mod stats;
use stats::handle_api_stats;

#[derive(Embed)]
#[folder = "src/dashboard_src/"]
pub struct DashboardAssets;

pub(crate) const STATS_CACHE_TTL: Duration = Duration::from_secs(2);
const DASHBOARD_WORKERS: usize = 4;
const DASHBOARD_QUEUE: usize = 32;

#[derive(Default)]
pub(crate) struct StatsCache {
    pub(crate) generated_at: Option<Instant>,
    pub(crate) data: Option<serde_json::Value>,
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

