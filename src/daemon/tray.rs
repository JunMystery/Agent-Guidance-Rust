use std::thread;

#[cfg(target_os = "windows")]
use super::tray_windows::run_windows_tray;

#[cfg(not(target_os = "windows"))]
use super::tray_unix::run_unix_tray;

/// Spawns the cross-platform system tray icon in a dedicated background thread.
pub fn spawn_system_tray(port: u16) {
    #[cfg(target_os = "windows")]
    {
        let _ = thread::Builder::new()
            .name("agent-guidance-tray".to_string())
            .spawn(move || {
                run_windows_tray(port);
            });
    }

    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(target_os = "linux")]
        {
            if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
                tracing::info!("Headless Linux environment detected, system tray disabled");
                return;
            }
        }

        let _ = thread::Builder::new()
            .name("agent-guidance-tray".to_string())
            .spawn(move || {
                run_unix_tray(port);
            });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_tray_thread_creation() {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::System::StationsAndDesktops::{OpenDesktopW, SetThreadDesktop};
            use windows_sys::Win32::UI::Shell::*;
            use windows_sys::Win32::UI::WindowsAndMessaging::*;

            unsafe {
                let default_name: Vec<u16> = "Default\0".encode_utf16().collect();
                let hdesk = OpenDesktopW(default_name.as_ptr(), 0, 0, 0x10000000);
                if hdesk != 0 {
                    SetThreadDesktop(hdesk);
                }

                let class_name: Vec<u16> = "TestTrayWnd\0".encode_utf16().collect();
                let mut wc: WNDCLASSW = std::mem::zeroed();
                wc.lpfnWndProc = Some(DefWindowProcW);
                wc.lpszClassName = class_name.as_ptr();
                RegisterClassW(&wc);

                let hwnd = CreateWindowExW(0, class_name.as_ptr(), class_name.as_ptr(), WS_OVERLAPPED, 0, 0, 0, 0, 0, 0, 0, std::ptr::null());
                assert_ne!(hwnd, 0);

                let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = 2001;
                nid.uFlags = NIF_MESSAGE | NIF_ICON;
                nid.hIcon = LoadIconW(0, IDI_APPLICATION);
                nid.uCallbackMessage = WM_USER + 1;

                let ret = Shell_NotifyIconW(NIM_ADD, &nid);
                assert_eq!(ret, 1, "Native Shell_NotifyIconW failed");
                Shell_NotifyIconW(NIM_DELETE, &nid);
                DestroyWindow(hwnd);
            }
        }
    }
}
