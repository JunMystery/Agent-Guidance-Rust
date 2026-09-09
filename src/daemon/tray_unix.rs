#[cfg(not(target_os = "windows"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_os = "windows"))]
use std::sync::Arc;
#[cfg(not(target_os = "windows"))]
use std::thread;
#[cfg(not(target_os = "windows"))]
use tray_item::{IconSource, TrayItem};

#[cfg(not(target_os = "windows"))]
use crate::daemon::tray_platform::{open_browser, open_folder};

#[cfg(not(target_os = "windows"))]
pub fn run_unix_tray(port: u16) {
    let mut tray = match TrayItem::new("Agent Guidance", IconSource::Resource("agent-guidance")) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("Unix tray init failed: {:?}", e);
            return;
        }
    };

    let _ = tray.add_label("Agent Guidance (MCP)");
    let url = format!("http://127.0.0.1:{}/", port);
    let _ = tray.add_menu_item("Open Dashboard", move || {
        open_browser(&url);
    });
    let _ = tray.add_menu_item("Open Data Directory", || {
        if let Ok(home) = std::env::var("HOME") {
            open_folder(std::path::Path::new(&home).join(".local/share/agent-guidance").as_path());
        }
    });
    let _ = tray.add_menu_item("Exit Daemon", || {
        std::process::exit(0);
    });

    let running = Arc::new(AtomicBool::new(true));
    while running.load(Ordering::Relaxed) {
        thread::sleep(std::time::Duration::from_secs(1));
    }
}
