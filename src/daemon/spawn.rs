use anyhow::Result;
use std::env;
use std::process::{Command, Stdio};
use tracing::info;

/// Spawns the agent-guidance daemon as a completely detached background process.
/// On Windows, sets DETACHED_PROCESS (0x00000008) and CREATE_NO_WINDOW (0x08000000)
/// so that closing the launching IDE never terminates the daemon.
pub fn ensure_daemon_running() -> Result<()> {
    let current_exe = env::current_exe()?;
    info!("Spawning background singleton daemon: {:?}", current_exe);

    let mut cmd = Command::new(&current_exe);
    cmd.arg("--daemon");
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    }

    cmd.spawn()?;
    info!("Detached singleton daemon process spawned successfully.");
    Ok(())
}
