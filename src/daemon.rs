use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;
use tracing::{error, info};

use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::mcp::router::handle_request;
use crate::mcp::state::ServerState;

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

    // Send client/connect metadata before forwarding JSON-RPC
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
        // Skip the connect response (first line from daemon)
        let _ = reader.next_line().await;
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
    let mut state = ServerState::with_client_name(client_name);

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
            error!("Request line exceeded max limit of {} bytes", MAX_LINE_BYTES);
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
                format!("Invalid Request: jsonrpc must be '2.0', got '{}'", request.jsonrpc),
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

        // P3: Timeout around handle_request to prevent indefinite blocking
        let result = match timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS), async {
            handle_request(&request.method, request.params, &mut state)
        }).await {
            Ok(res) => res,
            Err(_) => {
                error!("Request timed out after {}s: {}", REQUEST_TIMEOUT_SECS, request.method);
                if let Some(id) = req_id {
                    let resp = JsonRpcResponse::error(id, -32000, format!("Request timed out after {}s", REQUEST_TIMEOUT_SECS));
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
    // create_new(true) atomically creates the file only if it doesn't exist
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => {
            info!("Daemon lock acquired.");
            Some(file)
        }
        Err(_) => {
            info!("Another daemon is already running (lock exists).");
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

    // P0: Warm up ML models synchronously before accepting connections
    // Uses spawn_blocking so it doesn't starve the async runtime
    info!("Warming up ML models (this may take ~20s on first run)...");
    let warmup = tokio::task::spawn_blocking(|| {
        let _ = crate::ml::embeddings::warmup_cache();
        drop(crate::ml::llm_selector::cached_cross_encoder());
    });
    if let Err(e) = warmup.await {
        error!("Model warmup failed: {:?}", e);
    } else {
        info!("Model warmup complete.");
    }

    let socket_connections = Arc::new(AtomicUsize::new(0));
    let stdio_closed = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let sc = stdio_closed.clone();
    tokio::spawn(async move {
        info!("Handling initial stdio connection.");
        handle_mcp_lines(tokio::io::stdin(), tokio::io::stdout()).await;
        sc.store(true, Ordering::SeqCst);
    });

    let c_accept = socket_connections.clone();
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    c_accept.fetch_add(1, Ordering::SeqCst);
                    let c_sock = c_accept.clone();
                    tokio::spawn(async move {
                        let (reader, writer) = stream.into_split();
                        handle_mcp_lines(reader, writer).await;
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

    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if stdio_closed.load(Ordering::SeqCst) && socket_connections.load(Ordering::SeqCst) == 0 {
            info!("No active connections — daemon will shut down in 30s.");
            for remaining in (1..=30).rev() {
                if remaining % 10 == 0 {
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
