//! Daemon Client Lifecycle & Auto-Shutdown Monitor
//!
//! Monitors active IDE connections and system-wide IDE processes.
//! Keeps the singleton daemon resident in memory as long as at least one IDE is running.

use std::sync::atomic::Ordering;
use std::time::Duration;
use tracing::info;

use super::{ACTIVE_CLIENTS, CLIENT_NOTIFY};

pub const DEFAULT_IDLE_COOLDOWN_SECS: u64 = 60;
pub const DEFAULT_STARTUP_TIMEOUT_SECS: u64 = 30;

pub async fn monitor_client_lifecycle(startup_timeout_secs: u64) {
    let mut sys = sysinfo::System::new();
    let cooldown_secs: u64 = std::env::var("AGENT_GUIDANCE_IDLE_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_IDLE_COOLDOWN_SECS);

    monitor_client_lifecycle_with_checker(startup_timeout_secs, cooldown_secs, || {
        crate::daemon::has_running_ide_processes(&mut sys)
    })
    .await;
}

pub(crate) async fn monitor_client_lifecycle_with_checker<F>(
    startup_timeout_secs: u64,
    idle_cooldown_secs: u64,
    mut has_ides_fn: F,
) where
    F: FnMut() -> bool,
{
    let mut has_connected = false;
    let mut startup_elapsed = Duration::ZERO;
    let mut zero_ide_ticks = 0u64;
    let tick_interval = Duration::from_millis(500);
    let mut last_ide_log = tokio::time::Instant::now();

    loop {
        tokio::select! {
            _ = tokio::time::sleep(tick_interval) => {},
            _ = CLIENT_NOTIFY.notified() => {},
        }

        let active = ACTIVE_CLIENTS.load(Ordering::SeqCst);
        if active > 0 {
            has_connected = true;
            zero_ide_ticks = 0;
            continue;
        }

        if has_ides_fn() {
            zero_ide_ticks = 0;
            if last_ide_log.elapsed() >= Duration::from_secs(30) {
                info!("0 active MCP client connections, but IDE process(es) still active. Keeping daemon running.");
                last_ide_log = tokio::time::Instant::now();
            }
        } else if has_connected {
            zero_ide_ticks += 1;
            let secs_without_ides = zero_ide_ticks / 2;
            if secs_without_ides >= idle_cooldown_secs {
                info!(
                    "No IDEs detected for {}s (cooldown reached). Shutting down daemon cleanly.",
                    idle_cooldown_secs
                );
                break;
            }
        } else {
            startup_elapsed += tick_interval;
            if startup_elapsed.as_secs() >= startup_timeout_secs {
                info!(
                    "No IDE client connected within startup window ({}s). Shutting down daemon cleanly.",
                    startup_timeout_secs
                );
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::{client_connected, client_disconnected};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn test_daemon_stays_alive_while_ide_is_running() {
        let _guard = TEST_MUTEX.lock().unwrap();
        ACTIVE_CLIENTS.store(0, Ordering::SeqCst);
        let ide_running = Arc::new(AtomicBool::new(true));
        let ide_flag = ide_running.clone();

        let monitor_handle = tokio::spawn(async move {
            monitor_client_lifecycle_with_checker(10, 1, move || {
                ide_flag.load(Ordering::SeqCst)
            })
            .await;
        });

        // 1. Initial client connects and disconnects
        tokio::time::sleep(Duration::from_millis(50)).await;
        client_connected();
        assert_eq!(ACTIVE_CLIENTS.load(Ordering::SeqCst), 1);

        tokio::time::sleep(Duration::from_millis(50)).await;
        client_disconnected();
        assert_eq!(ACTIVE_CLIENTS.load(Ordering::SeqCst), 0);

        // 2. 0 active clients, but IDE is STILL RUNNING -> monitor MUST NOT finish!
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!monitor_handle.is_finished(), "Daemon MUST stay alive while at least 1 IDE is running");

        // 3. IDE terminates (0 IDEs remaining) -> monitor must finish after cooldown
        ide_running.store(false, Ordering::SeqCst);
        CLIENT_NOTIFY.notify_waiters();

        let completed = tokio::time::timeout(Duration::from_millis(1500), monitor_handle).await;
        assert!(completed.is_ok(), "Daemon must exit when 0 IDEs are running");
    }

    #[tokio::test]
    async fn test_daemon_multi_client_lifecycle() {
        let _guard = TEST_MUTEX.lock().unwrap();
        ACTIVE_CLIENTS.store(0, Ordering::SeqCst);

        let monitor_handle = tokio::spawn(async {
            monitor_client_lifecycle_with_checker(10, 1, || false).await;
        });

        // 1. Simulate Client 1 connects
        tokio::time::sleep(Duration::from_millis(50)).await;
        client_connected();
        assert_eq!(ACTIVE_CLIENTS.load(Ordering::SeqCst), 1);

        // 2. Simulate Client 2 connects
        tokio::time::sleep(Duration::from_millis(50)).await;
        client_connected();
        assert_eq!(ACTIVE_CLIENTS.load(Ordering::SeqCst), 2);

        // 3. Client 1 disconnects -> 1 active remains -> monitor must NOT finish
        tokio::time::sleep(Duration::from_millis(50)).await;
        client_disconnected();
        assert_eq!(ACTIVE_CLIENTS.load(Ordering::SeqCst), 1);
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!monitor_handle.is_finished(), "Daemon must still run while >= 1 client is active");

        // 4. Client 2 disconnects -> 0 active and 0 IDEs running -> monitor must finish
        client_disconnected();
        assert_eq!(ACTIVE_CLIENTS.load(Ordering::SeqCst), 0);

        let completed = tokio::time::timeout(Duration::from_millis(1500), monitor_handle).await;
        assert!(completed.is_ok(), "Monitor must exit when 0 active clients and 0 IDEs running");
    }
}
