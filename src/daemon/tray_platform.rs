use std::path::Path;
use std::process::Command;

/// Open a URL in default system browser
pub fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let op: Vec<u16> = "open\0".encode_utf16().collect();
        let url_wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            ShellExecuteW(
                0,
                op.as_ptr(),
                url_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

/// Open a directory in native file manager
pub fn open_folder(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer").arg(path).spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
}
