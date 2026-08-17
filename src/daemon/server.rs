use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tracing::{error, info};

#[cfg(unix)]
use tokio::net::UnixListener;

use super::{ACTIVE_CLIENTS, lock_path, socket_path};
use super::handler::handle_mcp_lines;

#[cfg(unix)]
fn acquire_daemon_lock() -> Option<std::fs::File> {
    let path = lock_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    // Use advisory flock so the lock auto-releases when the process dies
    match fs::OpenOptions::new().write(true).create(true).open(&path) {
        Ok(file) => {
            use fs2::FileExt;
            match file.try_lock_exclusive() {
                Ok(()) => {
                    info!("Daemon lock acquired.");
                    // Write PID for debugging
                    let _ = fs::write(&path, format!("{}", std::process::id()));
                    Some(file)
                }
                Err(_) => {
                    info!("Another daemon is already running (lock held).");
                    None
                }
            }
        }
        Err(e) => {
            error!("Failed to open daemon lock file: {}", e);
            None
        }
    }
}

#[cfg(unix)]
fn release_daemon_lock() {
    let path = lock_path();
    let _ = fs::remove_file(&path);
}

#[cfg(unix)]
pub async fn daemon_main() {
    // P2: Atomic lock to prevent dual-daemon race
    let _lock = match acquire_daemon_lock() {
        Some(l) => l,
        None => {
            error!("Daemon lock held by another process. Exiting.");
            std::process::exit(1);
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
    info!("Daemon listening on {:?}", path);

    // P0+: Background ML model warmup — daemon accepts connections immediately
    // Warmup runs in background; preloads Model and GPU Skill Matrix into VRAM
    info!("Starting background ML model and VRAM residency warmup...");
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

    let socket_connections = Arc::new(AtomicUsize::new(0));
    let stdio_closed = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let sc = stdio_closed.clone();
    tokio::spawn(async move {
        info!("Handling initial stdio connection.");
        ACTIVE_CLIENTS.fetch_add(1, Ordering::SeqCst);
        handle_mcp_lines(tokio::io::stdin(), tokio::io::stdout()).await;
        ACTIVE_CLIENTS.fetch_sub(1, Ordering::SeqCst);
        sc.store(true, Ordering::SeqCst);
    });

    let c_accept = socket_connections.clone();
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    c_accept.fetch_add(1, Ordering::SeqCst);
                    ACTIVE_CLIENTS.fetch_add(1, Ordering::SeqCst);
                    let c_sock = c_accept.clone();
                    tokio::spawn(async move {
                        let (reader, writer) = stream.into_split();
                        handle_mcp_lines(reader, writer).await;
                        ACTIVE_CLIENTS.fetch_sub(1, Ordering::SeqCst);
                        c_sock.fetch_sub(1, Ordering::SeqCst);
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
        .unwrap_or(600); // 10 minutes default

    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if stdio_closed.load(Ordering::SeqCst) && socket_connections.load(Ordering::SeqCst) == 0 {
            info!(
                "No active connections — daemon will shut down in {}s.",
                idle_timeout_secs
            );
            for remaining in (1..=idle_timeout_secs).rev() {
                if remaining % 60 == 0 || (remaining <= 10 && remaining % 2 == 0) {
                    info!("Shutdown in {}s...", remaining);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                if socket_connections.load(Ordering::SeqCst) > 0 {
                    info!("New connection arrived — cancelled shutdown.");
                    break;
                }
            }
            if socket_connections.load(Ordering::SeqCst) == 0 {
                info!("Idle timeout reached — shutting down daemon.");
                break;
            }
        }
    }

    let _ = fs::remove_file(&path);
    release_daemon_lock();
}

#[cfg(not(unix))]
pub async fn daemon_main() {
    // daemon mode not supported on non-unix platforms
}
