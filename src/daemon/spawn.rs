use anyhow::Result;
use std::env;
use std::process::{Command, Stdio};
use tracing::{info, warn};

/// Spawns the agent-guidance daemon as a completely detached background process,
/// fully decoupled from the launching IDE's process tree and Job Object.
pub fn ensure_daemon_running() -> Result<()> {
    let current_exe = env::current_exe()?;
    let exe_str = current_exe.to_string_lossy().to_string();
    info!("Ensuring background singleton daemon is running: {}", exe_str);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;

        // Attempt 1: Direct spawn with CREATE_BREAKAWAY_FROM_JOB
        let mut cmd = Command::new(&current_exe);
        cmd.arg("--daemon");
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        cmd.creation_flags(
            DETACHED_PROCESS
                | CREATE_NEW_PROCESS_GROUP
                | CREATE_NO_WINDOW
                | CREATE_BREAKAWAY_FROM_JOB,
        );

        match cmd.spawn() {
            Ok(_) => {
                info!("Detached singleton daemon spawned with breakaway from IDE job object.");
                return Ok(());
            }
            Err(e) => {
                warn!("Direct breakaway spawn failed ({}), attempting WMI detached spawn...", e);
            }
        }

        // Attempt 2: WMI Win32_Process.Create (Runs under Windows WMI Service, 100% outside IDE Job Object)
        let escaped_path = exe_str.replace("'", "''");
        let wmi_script = format!(
            "Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{{CommandLine = '\"{}\" --daemon'}}",
            escaped_path
        );

        let mut ps = Command::new("powershell");
        ps.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &wmi_script]);
        ps.stdin(Stdio::null());
        ps.stdout(Stdio::null());
        ps.stderr(Stdio::null());
        ps.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);

        match ps.status() {
            Ok(s) if s.success() => {
                info!("Detached singleton daemon spawned via WMI service.");
                return Ok(());
            }
            Err(e) => {
                warn!("WMI process spawn failed ({}), falling back to standard detached spawn...", e);
            }
            _ => {}
        }

        // Attempt 3: Standard detached spawn fallback
        let mut fallback = Command::new(&current_exe);
        fallback.arg("--daemon");
        fallback.stdin(Stdio::null());
        fallback.stdout(Stdio::null());
        fallback.stderr(Stdio::null());
        fallback.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
        fallback.spawn()?;
        info!("Detached singleton daemon spawned via standard detached fallback.");
        Ok(())
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut cmd = Command::new(&current_exe);
        cmd.arg("--daemon");
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        cmd.process_group(0);
        cmd.spawn()?;
        info!("Detached singleton daemon spawned with independent process group.");
        Ok(())
    }
}
