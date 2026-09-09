use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tracing::info;

use super::ensure_daemon_running;

#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use super::socket_path;

#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions;
#[cfg(windows)]
use super::WINDOWS_PIPE_NAME;

#[cfg(unix)]
pub async fn try_proxy_mode() -> bool {
    let path = socket_path();
    let mut spawned = false;
    for attempt in 0..30 {
        if path.exists() {
            match UnixStream::connect(&path).await {
                Ok(stream) => {
                    info!("Connected to background daemon via Unix socket: {:?}", path);
                    let (rx, tx) = stream.into_split();
                    proxy_stream(rx, tx).await;
                    return true;
                }
                Err(e) => {
                    if attempt == 0 && !spawned {
                        let _ = ensure_daemon_running();
                        spawned = true;
                    }
                    if attempt % 5 == 0 {
                        info!("Socket connect pending ({}), retrying...", e);
                    }
                }
            }
        } else if !spawned {
            let _ = ensure_daemon_running();
            spawned = true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    info!("Could not connect to background daemon socket.");
    false
}

#[cfg(windows)]
pub async fn try_proxy_mode() -> bool {
    let pipe_name = WINDOWS_PIPE_NAME;
    let mut spawned = false;
    for attempt in 0..30 {
        match ClientOptions::new().open(pipe_name) {
            Ok(client) => {
                info!("Connected to background daemon via Named Pipe: {}", pipe_name);
                let (rx, tx) = tokio::io::split(client);
                proxy_stream(rx, tx).await;
                return true;
            }
            Err(e) => {
                if !spawned {
                    let _ = ensure_daemon_running();
                    spawned = true;
                }
                if attempt % 5 == 0 {
                    info!("Named pipe connect pending ({}), retrying...", e);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    info!("Could not connect to background daemon Windows Named Pipe.");
    false
}

async fn proxy_stream<R, W>(socket_rx: R, mut socket_tx: W)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let to_daemon = async {
        let mut stdin = BufReader::new(tokio::io::stdin()).lines();
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
    };

    let to_stdout = async {
        let mut reader = BufReader::new(socket_rx).lines();
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
    };

    tokio::select! {
        _ = to_daemon => {},
        _ = to_stdout => {},
    }
}
