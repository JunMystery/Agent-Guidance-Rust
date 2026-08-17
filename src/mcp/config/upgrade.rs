use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::setup::run_setup;

const REPO: &str = "JunMystery/Agent-Guidance-Rust";

fn get_release_asset_name() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        Some("agent-guidance-windows-x86_64.zip")
    } else if cfg!(target_os = "linux") {
        Some("agent-guidance-linux-x86_64.tar.gz")
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            Some("agent-guidance-macos-aarch64.tar.gz")
        } else {
            Some("agent-guidance-macos-x86_64.tar.gz")
        }
    } else {
        None
    }
}

pub fn get_target_bin_path() -> Result<PathBuf> {
    let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("agent-guidance"));
    let target = if cfg!(windows) {
        dirs::data_local_dir()
            .map(|d| {
                d.join("Programs")
                    .join("agent-guidance")
                    .join("bin")
                    .join("agent-guidance.exe")
            })
            .unwrap_or(current_exe)
    } else {
        dirs::home_dir()
            .map(|h| h.join(".local").join("bin").join("agent-guidance"))
            .unwrap_or(current_exe)
    };
    Ok(target)
}

pub fn run_upgrade() -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: v{}", current_version);
    println!("Checking for latest release package from GitHub ({})...", REPO);

    let asset_name = get_release_asset_name()
        .ok_or_else(|| anyhow::anyhow!("Unsupported operating system or architecture for automatic upgrade"))?;

    let tmp_dir = env::temp_dir().join(format!("ag-upgrade-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir)?;

    let download_url = format!("https://github.com/{}/releases/latest/download/{}", REPO, asset_name);
    let archive_path = tmp_dir.join(asset_name);

    println!("  ↓ Downloading {}...", download_url);

    let downloaded = if cfg!(windows) {
        let ps_cmd = format!(
            "$ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing",
            download_url,
            archive_path.to_string_lossy().replace('\\', "/")
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
            .status();
        matches!(status, Ok(s) if s.success())
    } else {
        let status = std::process::Command::new("curl")
            .args(["-sSL", &download_url, "-o", &archive_path.to_string_lossy()])
            .status();
        match status {
            Ok(s) if s.success() => true,
            _ => {
                let wget_status = std::process::Command::new("wget")
                    .args(["-q", &download_url, "-O", &archive_path.to_string_lossy()])
                    .status();
                matches!(wget_status, Ok(s) if s.success())
            }
        }
    };

    if !downloaded || !archive_path.exists() || fs::metadata(&archive_path)?.len() == 0 {
        let _ = fs::remove_dir_all(&tmp_dir);
        anyhow::bail!("Failed to download release archive from {}", download_url);
    }

    println!("  ✓ Download complete. Extracting binary...");
    let extracted_bin = if cfg!(windows) {
        let ps_cmd = format!(
            "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
            archive_path.to_string_lossy().replace('\\', "/"),
            tmp_dir.to_string_lossy().replace('\\', "/")
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
            .status();
        if !matches!(status, Ok(s) if s.success()) {
            let _ = fs::remove_dir_all(&tmp_dir);
            anyhow::bail!("Failed to extract zip archive");
        }
        tmp_dir.join("agent-guidance.exe")
    } else {
        let status = std::process::Command::new("tar")
            .args(["-xzf", &archive_path.to_string_lossy(), "-C", &tmp_dir.to_string_lossy()])
            .status();
        if !matches!(status, Ok(s) if s.success()) {
            let _ = fs::remove_dir_all(&tmp_dir);
            anyhow::bail!("Failed to extract tar archive");
        }
        tmp_dir.join("agent-guidance")
    };

    if !extracted_bin.exists() {
        let _ = fs::remove_dir_all(&tmp_dir);
        anyhow::bail!("Extracted archive did not contain agent-guidance binary");
    }

    let target_bin = get_target_bin_path()?;
    if let Some(parent) = target_bin.parent() {
        fs::create_dir_all(parent)?;
    }

    println!("  ✓ Installing updated binary to {:?}", target_bin);

    // On Windows, if replacing a locked binary, move the existing binary out of the way first
    if target_bin.exists() {
        let old_backup = target_bin.with_extension("old");
        let _ = fs::remove_file(&old_backup);
        let _ = fs::rename(&target_bin, &old_backup);
    }

    fs::copy(&extracted_bin, &target_bin)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target_bin, perms)?;
    }

    let _ = fs::remove_dir_all(&tmp_dir);

    println!("✓ Successfully installed the latest release binary!");
    println!();
    println!("Configuring MCP server and syncing skills across all IDE clients...");
    run_setup(&target_bin)?;

    println!("✓ Agent Guidance successfully upgraded to the latest version!");
    Ok(())
}
