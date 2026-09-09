use fs2::FileExt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::{error, info};

use super::lock_path;

pub struct DaemonLock {
    file: std::fs::File,
    path: PathBuf,
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = fs::remove_file(&self.path);
    }
}

pub fn acquire_daemon_lock() -> Option<DaemonLock> {
    let path = lock_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    match fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
    {
        Ok(mut file) => match file.try_lock_exclusive() {
            Ok(()) => {
                info!("Daemon lock acquired.");
                let _ = file.set_len(0);
                let _ = write!(file, "{}", std::process::id());
                let _ = file.flush();
                Some(DaemonLock { file, path })
            }
            Err(_) => {
                info!("Another daemon is already running (lock held).");
                None
            }
        },
        Err(e) => {
            error!("Failed to open daemon lock file: {}", e);
            None
        }
    }
}
