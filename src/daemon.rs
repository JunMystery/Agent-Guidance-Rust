use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info};

use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::mcp::router::handle_request;
use crate::mcp::state::ServerState;

pub fn socket_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("agent-guidance")
        .join("mcp.sock")
}

pub async fn try_proxy_mode() -> bool {
    let path = socket_path();
    if !path.exists() {
        return false;
    }
    match UnixStream::connect(&path).await {
        Ok(stream) => {
            proxy_main(stream).await;
            true
        }
        Err(e) => {
            info!("Socket connect failed ({}), starting daemon mode.", e);
            false
        }
    }
}

async fn proxy_main(stream: UnixStream) {
    let (mut socket_rx, mut socket_tx) = stream.into_split();
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    let to_daemon = tokio::spawn(async move {
        let mut buf = [0u8; 65536];
        loop {
            match stdin.read(&mut buf).await {
                Ok(0) => {
                    let _ = socket_tx.shutdown().await;
                    break;
                }
                Ok(n) => {
                    if socket_tx.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let to_stdout = tokio::spawn(async move {
        let mut buf = [0u8; 65536];
        loop {
            match socket_rx.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if stdout.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                    let _ = stdout.flush().await;
                }
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
    let mut state = ServerState::new();

    const MAX_LINE_BYTES: usize = 10 * 1024 * 1024;

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
        match handle_request(&request.method, request.params, &mut state) {
            Ok(result) => {
                if let Some(id) = req_id {
                    let resp = JsonRpcResponse::success(id, result);
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

pub async fn daemon_main() {
    let path = socket_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            error!("Failed to create socket directory: {}", e);
            return;
        }
    }
    let _ = std::fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind socket: {}", e);
            return;
        }
    };
    info!("Daemon listening on {:?}", path);

    let connections = Arc::new(AtomicUsize::new(0));

    connections.fetch_add(1, Ordering::SeqCst);
    let c_stdio = connections.clone();
    tokio::spawn(async move {
        info!("Handling initial stdio connection.");
        handle_mcp_lines(tokio::io::stdin(), tokio::io::stdout()).await;
        c_stdio.fetch_sub(1, Ordering::SeqCst);
    });

    let c_accept = connections.clone();
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
        if connections.load(Ordering::SeqCst) == 0 {
            info!("No active connections — daemon will shut down in 30s.");
            for remaining in (1..=30).rev() {
                if remaining % 10 == 0 {
                    info!("Shutdown in {}s...", remaining);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                if connections.load(Ordering::SeqCst) > 0 {
                    info!("New connection arrived — cancelled shutdown.");
                    break;
                }
            }
            if connections.load(Ordering::SeqCst) == 0 {
                info!("Idle timeout reached — shutting down daemon.");
                break;
            }
        }
    }

    let _ = std::fs::remove_file(&path);
}
