use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{error, info};

use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::mcp::router::handle_request;
use crate::mcp::state::ServerState;

const MAX_REQUEST_WORKERS: usize = 4;
static REQUEST_WORKERS: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
pub static ACTIVE_CLIENTS: AtomicUsize = AtomicUsize::new(0);

pub fn active_clients_count() -> usize {
    ACTIVE_CLIENTS.load(Ordering::SeqCst)
}

#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("agent-guidance")
        .join("mcp.sock")
}

#[cfg(unix)]
fn lock_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("agent-guidance")
        .join("daemon.lock")
}

#[cfg(not(unix))]
pub fn socket_path() -> PathBuf {
    PathBuf::from("")
}

#[cfg(unix)]
pub async fn try_proxy_mode() -> bool {
    let path = socket_path();
    for attempt in 0..3 {
        if path.exists() {
            match UnixStream::connect(&path).await {
                Ok(stream) => {
                    proxy_main(stream).await;
                    return true;
                }
                Err(e) => {
                    info!("Socket connect failed ({}), retrying...", e);
                    // Stale socket — daemon died without cleanup. Remove it.
                    if attempt == 2 {
                        let _ = fs::remove_file(&path);
                        info!("Removed stale socket: {:?}", path);
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(200 * (attempt + 1))).await;
    }
    info!("Could not connect to existing daemon — will start new one.");
    false
}

#[cfg(not(unix))]
pub async fn try_proxy_mode() -> bool {
    false
}

#[cfg(unix)]
async fn proxy_main(stream: UnixStream) {
    let socket = stream;
    let sent_connect = std::env::var("AGENT_CLIENT_NAME").is_ok();

    if sent_connect {
        if let Ok(name) = std::env::var("AGENT_CLIENT_NAME") {
            let meta = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "client/connect",
                "params": { "name": name }
            });
            let meta_bytes = (serde_json::to_string(&meta).unwrap_or_default() + "\n").into_bytes();
            let _ = socket.writable().await;
            let _ = socket.try_write(&meta_bytes);
        }
    }

    let (socket_rx, socket_tx) = socket.into_split();

    let to_daemon = tokio::spawn(async move {
        let mut stdin = BufReader::new(tokio::io::stdin()).lines();
        let mut socket_tx = socket_tx;
        loop {
            match stdin.next_line().await {
                Ok(Some(line)) => {
                    let line_bytes = (line + "\n").into_bytes();
                    if socket_tx.write_all(&line_bytes).await.is_err() {
                        break;
                    }
                    if socket_tx.flush().await.is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = socket_tx.shutdown().await;
                    break;
                }
                Err(_) => break,
            }
        }
    });

    let to_stdout = tokio::spawn(async move {
        let mut reader = BufReader::new(socket_rx).lines();
        // Skip the connect response (first line from daemon) only if we sent one
        if sent_connect {
            let _ = reader.next_line().await;
        }
        let mut stdout = tokio::io::stdout();
        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    let line_bytes = (line + "\n").into_bytes();
                    if stdout.write_all(&line_bytes).await.is_err() {
                        break;
                    }
                    if stdout.flush().await.is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
    });

    let _ = tokio::join!(to_daemon, to_stdout);
}

pub async fn handle_mcp_lines<R, W>(reader: R, mut writer: W)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
    W: tokio::io::AsyncWrite + Unpin + Send,
{
    let mut reader = BufReader::new(reader).lines();
    let client_name = std::env::var("AGENT_CLIENT_NAME").ok();
    let state = Arc::new(Mutex::new(ServerState::with_client_name(client_name)));

    const MAX_LINE_BYTES: usize = 10 * 1024 * 1024;
    const REQUEST_TIMEOUT_SECS: u64 = 60;

    loop {
        let line = match reader.next_line().await {
            Ok(Some(l)) => l,
            Ok(None) => break,
            Err(e) => {
                error!("MCP read error: {}", e);
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        if line.len() > MAX_LINE_BYTES {
            error!(
                "Request line exceeded max limit of {} bytes",
                MAX_LINE_BYTES
            );
            let err_resp = JsonRpcResponse::error(
                serde_json::Value::Null,
                -32600,
                "Invalid Request: payload too large",
            );
            let out = serde_json::to_string(&err_resp).unwrap_or_default() + "\n";
            if let Err(e) = writer.write_all(out.as_bytes()).await {
                error!("Write error: {}", e);
                break;
            }
            if let Err(e) = writer.flush().await {
                error!("Flush error: {}", e);
                break;
            }
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                error!("Invalid JSON-RPC request: {}", e);
                let err_resp = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    -32700,
                    format!("Parse error: {}", e),
                );
                let out = serde_json::to_string(&err_resp).unwrap_or_default() + "\n";
                if let Err(e) = writer.write_all(out.as_bytes()).await {
                    error!("Write error: {}", e);
                    break;
                }
                if let Err(e) = writer.flush().await {
                    error!("Flush error: {}", e);
                    break;
                }
                continue;
            }
        };

        if request.jsonrpc != "2.0" {
            error!("Unsupported JSON-RPC version: {}", request.jsonrpc);
            let id = request.id.clone().unwrap_or(serde_json::Value::Null);
            let err_resp = JsonRpcResponse::error(
                id,
                -32600,
                format!(
                    "Invalid Request: jsonrpc must be '2.0', got '{}'",
                    request.jsonrpc
                ),
            );
            let out = serde_json::to_string(&err_resp).unwrap_or_default() + "\n";
            if let Err(e) = writer.write_all(out.as_bytes()).await {
                error!("Write error: {}", e);
                break;
            }
            if let Err(e) = writer.flush().await {
                error!("Flush error: {}", e);
                break;
            }
            continue;
        }

        let req_id = request.id.clone();
        let request_method = request.method.clone();
        let timeout_method = request_method.clone();
        let request_params = request.params;
        let request_state = state.clone();
        let cancellation = Arc::new(AtomicBool::new(false));
        let worker_cancellation = cancellation.clone();

        // Keep synchronous parsing, filesystem, indexing, and ML work off Tokio's I/O workers.
        let worker_pool = REQUEST_WORKERS
            .get_or_init(|| Arc::new(Semaphore::new(MAX_REQUEST_WORKERS)))
            .clone();
        let permit = match timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECS),
            worker_pool.acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                error!("Request worker pool closed");
                continue;
            }
            Err(_) => {
                error!(
                    "Request worker pool wait exceeded {}s",
                    REQUEST_TIMEOUT_SECS
                );
                if let Some(id) = req_id.clone() {
                    let resp =
                        JsonRpcResponse::error(id, -32000, "Request worker pool is saturated");
                    let out = serde_json::to_string(&resp).unwrap_or_default() + "\n";
                    let _ = writer.write_all(out.as_bytes()).await;
                    let _ = writer.flush().await;
                }
                continue;
            }
        };

        let is_read = crate::mcp::router::is_read_only_request(&request_method, &request_params);

        let result = match timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECS),
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let started = Instant::now();
                let result = if is_read {
                    let mut state_clone = {
                        let state_guard = request_state
                            .lock()
                            .map_err(|_| (-32000, "Server state lock poisoned".to_string()))?;
                        state_guard.clone()
                    };
                    state_clone.set_cancellation(worker_cancellation);
                    let res = handle_request(&request_method, request_params, &mut state_clone);
                    state_clone.clear_cancellation();
                    res
                } else {
                    let mut state_guard = request_state
                        .lock()
                        .map_err(|_| (-32000, "Server state lock poisoned".to_string()))?;
                    state_guard.set_cancellation(worker_cancellation);
                    let res = handle_request(&request_method, request_params, &mut *state_guard);
                    state_guard.clear_cancellation();
                    res
                };
                info!(
                    method = %request_method,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "MCP request completed"
                );
                result
            }),
        )
        .await
        {
            Ok(Ok(res)) => res,
            Ok(Err(e)) => Err((-32000, format!("Request worker failed: {}", e))),
            Err(_) => {
                cancellation.store(true, Ordering::Relaxed);
                error!(
                    "Request timed out after {}s: {}",
                    REQUEST_TIMEOUT_SECS, timeout_method
                );
                if let Some(id) = req_id {
                    let resp = JsonRpcResponse::error(
                        id,
                        -32000,
                        format!("Request timed out after {}s", REQUEST_TIMEOUT_SECS),
                    );
                    let out = serde_json::to_string(&resp).unwrap_or_default() + "\n";
                    let _ = writer.write_all(out.as_bytes()).await;
                    let _ = writer.flush().await;
                }
                continue;
            }
        };

        match result {
            Ok(resp_value) => {
                if let Some(id) = req_id {
                    let resp = JsonRpcResponse::success(id, resp_value);
                    let out = serde_json::to_string(&resp).unwrap_or_default() + "\n";
                    if let Err(e) = writer.write_all(out.as_bytes()).await {
                        error!("Write error: {}", e);
                        break;
                    }
                    if let Err(e) = writer.flush().await {
                        error!("Flush error: {}", e);
                        break;
                    }
                }
            }
            Err((code, msg)) => {
                if let Some(id) = req_id {
                    let resp = JsonRpcResponse::error(id, code, msg);
                    let out = serde_json::to_string(&resp).unwrap_or_default() + "\n";
                    if let Err(e) = writer.write_all(out.as_bytes()).await {
                        error!("Write error: {}", e);
                        break;
                    }
                    if let Err(e) = writer.flush().await {
                        error!("Flush error: {}", e);
                        break;
                    }
                }
            }
        }
    }
}

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
    // Warmup runs in background; first ML search calls use keyword fallback until ready
    info!("Starting background ML model warmup (disk cache or ~20s compute)...");
    tokio::spawn(async {
        let warmup = tokio::task::spawn_blocking(|| {
            crate::ml::embeddings::warmup_cache();
        });
        if let Err(e) = warmup.await {
            error!("Model warmup failed: {:?}", e);
        } else {
            info!("Background model warmup completed.");
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
