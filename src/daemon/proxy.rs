use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::info;

#[cfg(unix)]
use tokio::net::UnixStream;

use super::socket_path;

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
