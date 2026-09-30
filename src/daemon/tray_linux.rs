#![cfg(target_os = "linux")]

use crate::daemon::tray_platform::{open_browser, open_folder};
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, ToolTip, Tray, TrayService};

const LOGO_PNG: &[u8] = include_bytes!("../../docs/images/logo.png");

struct LinuxTray {
    port: u16,
}

impl Tray for LinuxTray {
    fn id(&self) -> String {
        "agent-guidance-mcp".into()
    }

    fn title(&self) -> String {
        "Agent Guidance (MCP)".into()
    }

    fn icon_name(&self) -> String {
        "agent-guidance".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Agent Guidance (MCP)".into(),
            description: "Running in background".into(),
            ..Default::default()
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let Ok(img) = image::load_from_memory(LOGO_PNG) else {
            return Vec::new();
        };

        let decoded = img.into_rgba8();
        let (width, height) = decoded.dimensions();
        let raw_rgba = decoded.into_raw();

        // FreeDesktop SNI: ARGB32 in network byte order (Alpha, Red, Green, Blue)
        let mut argb_data = Vec::with_capacity(raw_rgba.len());
        for chunk in raw_rgba.chunks_exact(4) {
            argb_data.push(chunk[3]); // Alpha
            argb_data.push(chunk[0]); // Red
            argb_data.push(chunk[1]); // Green
            argb_data.push(chunk[2]); // Blue
        }

        vec![Icon {
            width: width as i32,
            height: height as i32,
            data: argb_data,
        }]
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let url = format!("http://127.0.0.1:{}/", self.port);
        let url_clone = url.clone();

        vec![
            StandardItem {
                label: "Open Web Dashboard".into(),
                activate: Box::new(move |_| {
                    open_browser(&url_clone);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Data Folder".into(),
                activate: Box::new(|_| {
                    if let Ok(home) = std::env::var("HOME") {
                        open_folder(
                            std::path::Path::new(&home)
                                .join(".local/share/agent-guidance")
                                .as_path(),
                        );
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Exit MCP Daemon".into(),
                activate: Box::new(|_| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let url = format!("http://127.0.0.1:{}/", self.port);
        open_browser(&url);
    }
}

pub fn run_linux_tray(port: u16) {
    let mut backoff = std::time::Duration::from_secs(2);

    loop {
        let start = std::time::Instant::now();
        let tray = LinuxTray { port };
        let service = TrayService::new(tray);

        let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            service.run()
        }));

        match run_result {
            Ok(Ok(())) => {
                tracing::info!("Linux system tray service stopped cleanly");
                break;
            }
            Ok(Err(e)) => {
                tracing::warn!("Linux system tray D-Bus service exited: {e}; retrying in {backoff:?}");
            }
            Err(panic_err) => {
                let panic_msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_err.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                tracing::warn!(
                    "Linux system tray caught panic: {panic_msg}; retrying in {backoff:?}"
                );
            }
        }

        std::thread::sleep(backoff);
        if start.elapsed() > std::time::Duration::from_secs(30) {
            backoff = std::time::Duration::from_secs(2);
        } else {
            backoff = (backoff * 2).min(std::time::Duration::from_secs(60));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_tray_metadata() {
        let tray = LinuxTray { port: 8080 };
        assert_eq!(tray.id(), "agent-guidance-mcp");
        assert_eq!(tray.title(), "Agent Guidance (MCP)");
        assert_eq!(tray.icon_name(), "agent-guidance");

        let tip = tray.tool_tip();
        assert_eq!(tip.title, "Agent Guidance (MCP)");
        assert_eq!(tip.description, "Running in background");

        let menu = tray.menu();
        assert_eq!(menu.len(), 4);

        let icons = tray.icon_pixmap();
        assert_eq!(icons.len(), 1);
        assert!(icons[0].width > 0);
        assert!(icons[0].height > 0);
        assert_eq!(
            icons[0].data.len(),
            (icons[0].width * icons[0].height * 4) as usize
        );
    }
}
