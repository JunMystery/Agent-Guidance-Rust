use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod proxy;
pub mod handler;
pub mod server;

pub use proxy::try_proxy_mode;
pub use handler::handle_mcp_lines;
pub use server::daemon_main;

pub static ACTIVE_CLIENTS: AtomicUsize = AtomicUsize::new(0);

pub fn active_clients_count() -> usize {
    ACTIVE_CLIENTS.load(Ordering::SeqCst)
}

#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("agent-guidance")
        .join("mcp.sock")
}

#[cfg(windows)]
pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\agent-guidance-mcp";

pub(crate) fn lock_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("agent-guidance")
        .join("daemon.lock")
}
