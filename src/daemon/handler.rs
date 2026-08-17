use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, AsyncRead, AsyncWrite, BufReader};
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{error, info};

use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::mcp::router::handle_request;
use crate::mcp::state::ServerState;
use super::ACTIVE_CLIENTS;

const MAX_REQUEST_WORKERS: usize = 4;
static REQUEST_WORKERS: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();

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
