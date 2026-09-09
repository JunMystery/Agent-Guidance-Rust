use std::sync::atomic::Ordering;
use std::time::Duration;
use tracing::{error, info};

use super::{
    ACTIVE_CLIENTS, CLIENT_NOTIFY, acquire_daemon_lock, client_connected, client_disconnected,
};
use super::handler::handle_mcp_lines;

use super::lifecycle::{DEFAULT_STARTUP_TIMEOUT_SECS, monitor_client_lifecycle};

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
                    client_connected();
                    tokio::spawn(async move {
                        let (reader, writer) = stream.into_split();
                        handle_mcp_lines(reader, writer).await;
                        client_disconnected();
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                    break;
                }
            }
        }
    });

    let startup_timeout_secs: u64 = std::env::var("AGENT_GUIDANCE_IDLE_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_STARTUP_TIMEOUT_SECS);

    monitor_client_lifecycle(startup_timeout_secs).await;
    let _ = fs::remove_file(&path);
}

#[cfg(windows)]
pub async fn daemon_main(port: u16, project_path: Option<String>) {
    crate::daemon::tray::spawn_system_tray(port);
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
                    client_connected();
                    tokio::spawn(async move {
                        let (reader, writer) = tokio::io::split(server);
                        handle_mcp_lines(reader, writer).await;
                        client_disconnected();
                    });
                }
                Err(e) => {
                    error!("Named pipe connect error: {}", e);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    });

    let startup_timeout_secs: u64 = std::env::var("AGENT_GUIDANCE_IDLE_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_STARTUP_TIMEOUT_SECS);

    monitor_client_lifecycle(startup_timeout_secs).await;
}
