use anyhow::Result;
use rust_embed::Embed;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, mpsc::sync_channel};
use std::time::{Duration, Instant};
use tiny_http::Server;
use tracing::info;

pub mod graph;
pub(crate) mod graph_query;
pub mod logs_api;
pub mod projects;
pub(crate) mod projects_path;
pub(crate) mod projects_prune;
pub(crate) mod router;
pub(crate) use router::json_response;
pub mod stats;
pub mod stats_query;
pub(crate) mod stats_query_aggregates;

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
                router::handle_dashboard_request(request, &project_path, &cache);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    static PORT_TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_port_is_11997() {
        let _guard = PORT_TEST_MUTEX.lock().unwrap();
        DASHBOARD_PORT.store(DEFAULT_DASHBOARD_PORT, Ordering::SeqCst);
        assert_eq!(DEFAULT_DASHBOARD_PORT, 11997);
        assert_eq!(get_dashboard_port(), 11997);
    }

    #[test]
    fn test_custom_dashboard_port_mutation() {
        let _guard = PORT_TEST_MUTEX.lock().unwrap();
        DASHBOARD_PORT.store(12345, Ordering::SeqCst);
        assert_eq!(get_dashboard_port(), 12345);
        DASHBOARD_PORT.store(DEFAULT_DASHBOARD_PORT, Ordering::SeqCst);
        assert_eq!(get_dashboard_port(), 11997);
    }

    #[test]
    fn test_favicon_assets_embedded() {
        let ico = include_bytes!("../../docs/images/logo.ico");
        let png = include_bytes!("../../docs/images/logo.png");
        assert!(!ico.is_empty());
        assert!(!png.is_empty());
    }
}
