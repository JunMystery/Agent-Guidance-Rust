use std::sync::atomic::Ordering;
use std::time::Duration;
use tracing::{error, info};

use super::{ACTIVE_CLIENTS, acquire_daemon_lock};
use super::handler::handle_mcp_lines;

pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 60;

async fn monitor_idle_cooldown(timeout_secs: u64) {
    let mut idle_elapsed = 0u64;
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let active = ACTIVE_CLIENTS.load(Ordering::SeqCst);
        if active == 0 {
            idle_elapsed += 1;
            if idle_elapsed % 15 == 0 || (timeout_secs.saturating_sub(idle_elapsed) <= 10 && idle_elapsed % 2 == 0) {
                info!(
                    "No active IDE clients connected. Daemon shutdown in {}s...",
                    timeout_secs.saturating_sub(idle_elapsed)
                );
            }
            if idle_elapsed >= timeout_secs {
                info!(
                    "Idle cooldown reached ({}s with 0 active IDEs). Shutting down singleton daemon cleanly.",
                    timeout_secs
                );
                break;
            }
        } else if idle_elapsed > 0 {
            info!(
                "Active IDE client reconnected ({} active). Cancelled shutdown cooldown.",
                active
            );
            idle_elapsed = 0;
        }
    }
}

#[cfg(unix)]
pub async fn daemon_main(port: u16, project_path: Option<String>) {
    use std::fs;
    use tokio::net::UnixListener;
    use super::socket_path;

    let _lock = match acquire_daemon_lock() {
        Some(l) => l,
        None => {
            info!("Daemon lock held by another process. Exiting.");
            return;
        }
    };

    let path = socket_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            error!("Failed to create socket directory: {}", e);
            return;
        }
    }
    let _ = fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind socket: {}", e);
            return;
        }
    };
    info!("Daemon listening on Unix socket: {:?}", path);

    // Background ML model warmup
    tokio::spawn(async {
        let warmup = tokio::task::spawn_blocking(|| {
            let _ = crate::ml::embeddings::eager_vram_warmup();
        });
        if let Err(e) = warmup.await {
            error!("Model warmup failed: {:?}", e);
        } else {
            info!("Background VRAM model & skill matrix residency warmup completed.");
        }
    });

    crate::dashboard::spawn_dashboard_background(port, project_path);

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    ACTIVE_CLIENTS.fetch_add(1, Ordering::SeqCst);
                    tokio::spawn(async move {
                        let (reader, writer) = stream.into_split();
                        handle_mcp_lines(reader, writer).await;
                        ACTIVE_CLIENTS.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                    break;
                }
            }
        }
    });

    let idle_timeout_secs: u64 = std::env::var("AGENT_GUIDANCE_IDLE_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_IDLE_TIMEOUT_SECS);

    monitor_idle_cooldown(idle_timeout_secs).await;
    let _ = fs::remove_file(&path);
}

#[cfg(windows)]
pub async fn daemon_main(port: u16, project_path: Option<String>) {
    use tokio::net::windows::named_pipe::ServerOptions;
    use super::WINDOWS_PIPE_NAME;

    let _lock = match acquire_daemon_lock() {
        Some(l) => l,
        None => {
            info!("Daemon lock held by another process. Exiting.");
            return;
        }
    };

    let pipe_name = WINDOWS_PIPE_NAME;
    info!("Daemon listening on Windows Named Pipe: {}", pipe_name);

    // Background ML model warmup
    tokio::spawn(async {
        let warmup = tokio::task::spawn_blocking(|| {
            let _ = crate::ml::embeddings::eager_vram_warmup();
        });
        if let Err(e) = warmup.await {
            error!("Model warmup failed: {:?}", e);
        } else {
            info!("Background VRAM model & skill matrix residency warmup completed.");
        }
    });

    crate::dashboard::spawn_dashboard_background(port, project_path);

    tokio::spawn(async move {
        let mut is_first = true;
        loop {
            let server_res = ServerOptions::new()
                .first_pipe_instance(is_first)
                .create(pipe_name);

            let server = match server_res {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create named pipe instance: {}", e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };
            is_first = false;

            match server.connect().await {
                Ok(()) => {
                    ACTIVE_CLIENTS.fetch_add(1, Ordering::SeqCst);
                    tokio::spawn(async move {
                        let (reader, writer) = tokio::io::split(server);
                        handle_mcp_lines(reader, writer).await;
                        ACTIVE_CLIENTS.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) => {
                    error!("Named pipe connect error: {}", e);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    });

    let idle_timeout_secs: u64 = std::env::var("AGENT_GUIDANCE_IDLE_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_IDLE_TIMEOUT_SECS);

    monitor_idle_cooldown(idle_timeout_secs).await;
}
