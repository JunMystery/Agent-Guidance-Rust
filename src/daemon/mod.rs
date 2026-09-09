use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use tokio::sync::Notify;
use tracing::info;

pub mod proxy;
pub mod handler;
pub mod server;
pub mod lock;
pub mod spawn;
pub mod tray;
pub mod tray_windows;
pub mod tray_unix;
pub mod tray_platform;
pub mod ide_detector;
pub mod lifecycle;

pub use proxy::try_proxy_mode;
pub use handler::handle_mcp_lines;
pub use server::daemon_main;
pub use lock::{acquire_daemon_lock, DaemonLock};
pub use spawn::ensure_daemon_running;
pub use ide_detector::{count_running_ide_processes, has_running_ide_processes, is_ide_process_name};
pub use lifecycle::monitor_client_lifecycle;

pub static ACTIVE_CLIENTS: AtomicUsize = AtomicUsize::new(0);
pub static CLIENT_NOTIFY: LazyLock<Notify> = LazyLock::new(Notify::new);

pub fn active_clients_count() -> usize {
    ACTIVE_CLIENTS.load(Ordering::SeqCst)
}

pub fn client_connected() {
    let count = ACTIVE_CLIENTS.fetch_add(1, Ordering::SeqCst) + 1;
    info!("IDE client connected ({} active).", count);
    CLIENT_NOTIFY.notify_waiters();
}

pub fn client_disconnected() {
    let prev = ACTIVE_CLIENTS.fetch_sub(1, Ordering::SeqCst);
    let count = prev.saturating_sub(1);
    info!("IDE client disconnected ({} active).", count);
    CLIENT_NOTIFY.notify_waiters();
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
