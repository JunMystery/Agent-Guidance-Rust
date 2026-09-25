pub mod handlers;
pub mod router;
pub mod state;
pub mod types;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use anyhow::{Context, Result};
use tiny_http::Server;
use tracing::info;

pub use state::WorkerState;

pub const DEFAULT_WORKER_PORT: u16 = 11998;
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1";
const WORKER_THREADS: usize = 4;

pub fn run_worker_server(bind_addr: &str, port: u16, api_key: String) -> Result<()> {
    let addr = format!("{}:{}", bind_addr, port);
    let server = Arc::new(
        Server::http(&addr).map_err(|e| anyhow::anyhow!("Failed to bind ML Worker on {}: {}", addr, e))?,
    );
    info!("[ML Worker Daemon] Listening on http://{}", addr);

    // Load or compile initial state
    let state = WorkerState::load_from_disk().unwrap_or_else(|_| {
        info!("Initial binaries missing on disk; running one-time compilation...");
        let _ = crate::ml::embeddings::compiler::compile_staging_to_binary(false);
        WorkerState::load_from_disk().unwrap_or_else(|_| WorkerState::new_empty())
    });

    let mut handles = Vec::new();
    for thread_id in 0..WORKER_THREADS {
        let s = Arc::clone(&server);
        let st = state.clone();
        let key = api_key.clone();
        let handle = std::thread::Builder::new()
            .name(format!("ml-worker-thread-{}", thread_id))
            .spawn(move || {
                for req in s.incoming_requests() {
                    router::route_request(req, &st, &key);
                }
            })
            .context("Failed to spawn ML worker thread")?;
        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }
    Ok(())
}

pub fn spawn_worker_background(bind_addr: String, port: u16, api_key: String) {
    let _ = std::thread::Builder::new()
        .name("ml-worker-background".to_string())
        .spawn(move || {
            if let Err(e) = run_worker_server(&bind_addr, port, api_key) {
                tracing::warn!("ML Worker background server exited: {}", e);
            }
        });
}
