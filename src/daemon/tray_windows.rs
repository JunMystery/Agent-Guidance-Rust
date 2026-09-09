#[cfg(target_os = "windows")]
use crate::daemon::tray_platform::{open_browser, open_folder};

#[cfg(target_os = "windows")]
const LOGO_PNG: &[u8] = include_bytes!("../../docs/images/logo.png");

#[cfg(target_os = "windows")]
pub fn run_windows_tray(port: u16) {
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::StationsAndDesktops::{OpenDesktopW, SetThreadDesktop};
    use windows_sys::Win32::UI::Shell::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let default_name: Vec<u16> = "Default\0".encode_utf16().collect();
        let hdesk = OpenDesktopW(default_name.as_ptr(), 0, 0, 0x10000000);
        if hdesk != 0 {
            SetThreadDesktop(hdesk);
        }

        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name: Vec<u16> = "AgentGuidanceTrayWnd\0".encode_utf16().collect();

        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(tray_wnd_proc);
        wc.hInstance = hinstance;
        wc.lpszClassName = class_name.as_ptr();
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            0,
            0,
            hinstance,
            std::ptr::null(),
        );

        if hwnd == 0 {
            tracing::warn!("Failed to create tray hidden window: {}", GetLastError());
            return;
        }

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, port as isize);

        let mut icon = LoadIconW(hinstance, 1 as *const u16);
        if icon == 0 {
            icon = CreateIconFromResourceEx(
                LOGO_PNG.as_ptr() as *mut u8,
                LOGO_PNG.len() as u32,
                1,
                0x00030000,
                32,
                32,
                LR_DEFAULTCOLOR,
            );
        }
        if icon == 0 {
            icon = LoadIconW(0, IDI_APPLICATION);
        }

        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1001;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.hIcon = icon;
        nid.uCallbackMessage = WM_USER + 100;

        let tip: Vec<u16> = "Agent Guidance (MCP)\0".encode_utf16().collect();
        let copy_len = tip.len().min(nid.szTip.len());
        nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);

        let added = Shell_NotifyIconW(NIM_ADD, &nid);
        if added == 0 {
            tracing::warn!("Shell_NotifyIconW failed: {}", GetLastError());
            DestroyWindow(hwnd);
            return;
        }
        tracing::info!("Tray icon successfully added to taskbar notification area!");

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn tray_wnd_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const WM_TRAY_CALLBACK: u32 = WM_USER + 100;
    const IDM_DASHBOARD: usize = 2001;
    const IDM_DATA_DIR: usize = 2002;
    const IDM_EXIT: usize = 2003;

    unsafe {
        if msg == WM_TRAY_CALLBACK {
            let event = lparam as u32;
            if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                let mut pt: POINT = std::mem::zeroed();
                GetCursorPos(&mut pt);
                SetForegroundWindow(hwnd);

                let menu = CreatePopupMenu();
                let label: Vec<u16> = "Agent Guidance (Running)\0".encode_utf16().collect();
                let dash: Vec<u16> = "Open Web Dashboard\0".encode_utf16().collect();
                let folder: Vec<u16> = "Open Data Folder\0".encode_utf16().collect();
                let exit_txt: Vec<u16> = "Exit MCP Daemon\0".encode_utf16().collect();

                AppendMenuW(menu, MF_GRAYED | MF_STRING, 0, label.as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(menu, MF_STRING, IDM_DASHBOARD, dash.as_ptr());
                AppendMenuW(menu, MF_STRING, IDM_DATA_DIR, folder.as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(menu, MF_STRING, IDM_EXIT, exit_txt.as_ptr());

                let port = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u16;

                let cmd = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                    pt.x,
                    pt.y,
                    0,
                    hwnd,
                    std::ptr::null(),
                ) as usize;

                DestroyMenu(menu);

                if cmd == IDM_DASHBOARD {
                    open_browser(&format!("http://127.0.0.1:{}/", port));
                } else if cmd == IDM_DATA_DIR {
                    if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
                        open_folder(std::path::Path::new(&appdata).join("agent-guidance").as_path());
                    }
                } else if cmd == IDM_EXIT {
                    std::process::exit(0);
                }
            }
            return 0;
        }

        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
