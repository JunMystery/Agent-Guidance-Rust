use anyhow::Result;
use std::env;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod catalog;
mod context;
mod mcp;
mod ml;
mod optimizer;

use mcp::config::run_setup;
use mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use mcp::router::handle_request;
use mcp::state::ServerState;

#[tokio::main]
async fn main() -> Result<()> {
    // Check command line arguments for --setup flag
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--setup".to_string()) {
        let exe_path = env::current_exe()?;
        let target_bin = dirs::home_dir()
            .map(|h| h.join(".local").join("bin").join(if cfg!(windows) { "agent-guidance.exe" } else { "agent-guidance" }))
            .unwrap_or(exe_path);
        
        run_setup(&target_bin)?;
        println!("✓ Agent Guidance Rust MCP server configured in all IDE clients successfully!");
        return Ok(());
    }

    // Initialize logging to stderr (never stdout to avoid corrupting MCP JSON-RPC protocol)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    info!("Starting Agent Guidance MCP Rust Server v{}", env!("CARGO_PKG_VERSION"));

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();
    let mut state = ServerState::new();

    const MAX_LINE_BYTES: usize = 10 * 1024 * 1024; // 10MB max per line

    while let Some(line) = reader.next_line().await? {
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
            let out = serde_json::to_string(&err_resp)? + "\n";
            stdout.write_all(out.as_bytes()).await?;
            stdout.flush().await?;
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
                let out = serde_json::to_string(&err_resp)? + "\n";
                stdout.write_all(out.as_bytes()).await?;
                stdout.flush().await?;
                continue;
            }
        };

        if request.jsonrpc != "2.0" {
            error!("Unsupported JSON-RPC version: {}", request.jsonrpc);
            let id = request.id.unwrap_or(serde_json::Value::Null);
            let err_resp = JsonRpcResponse::error(
                id,
                -32600,
                format!("Invalid Request: jsonrpc must be '2.0', got '{}'", request.jsonrpc),
            );
            let out = serde_json::to_string(&err_resp)? + "\n";
            stdout.write_all(out.as_bytes()).await?;
            stdout.flush().await?;
            continue;
        }

        let req_id = request.id.clone();
        match handle_request(&request.method, request.params, &mut state) {
            Ok(result) => {
                if let Some(id) = req_id {
                    let resp = JsonRpcResponse::success(id, result);
                    let out = serde_json::to_string(&resp)? + "\n";
                    stdout.write_all(out.as_bytes()).await?;
                    stdout.flush().await?;
                }
            }
            Err((code, msg)) => {
                if let Some(id) = req_id {
                    let resp = JsonRpcResponse::error(id, code, msg);
                    let out = serde_json::to_string(&resp)? + "\n";
                    stdout.write_all(out.as_bytes()).await?;
                    stdout.flush().await?;
                }
            }
        }
    }

    info!("Agent Guidance MCP Rust Server shutting down cleanly.");
    Ok(())
}
