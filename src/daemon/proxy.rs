use std::fs;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tracing::info;

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
    for attempt in 0..3 {
        if path.exists() {
            match UnixStream::connect(&path).await {
                Ok(stream) => {
                    let (rx, tx) = stream.into_split();
                    proxy_stream(rx, tx).await;
                    return true;
                }
                Err(e) => {
                    info!("Socket connect failed ({}), retrying...", e);
                    if attempt == 2 {
                        let _ = fs::remove_file(&path);
                        info!("Removed stale socket: {:?}", path);
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(150 * (attempt + 1))).await;
    }
    info!("Could not connect to existing daemon — will start new one.");
    false
}

#[cfg(windows)]
pub async fn try_proxy_mode() -> bool {
    let pipe_name = WINDOWS_PIPE_NAME;
    for attempt in 0..3 {
        match ClientOptions::new().open(pipe_name) {
            Ok(client) => {
                info!("Connected to background daemon via Named Pipe: {}", pipe_name);
                let (rx, tx) = tokio::io::split(client);
                proxy_stream(rx, tx).await;
                return true;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(100 * (attempt + 1))).await;
            }
        }
    }
    info!("Could not connect to existing Windows daemon pipe — will start new daemon.");
    false
}

async fn proxy_stream<R, W>(socket_rx: R, mut socket_tx: W)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let to_daemon = tokio::spawn(async move {
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
    });

    let to_stdout = tokio::spawn(async move {
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
    });

    let _ = tokio::join!(to_daemon, to_stdout);
}
