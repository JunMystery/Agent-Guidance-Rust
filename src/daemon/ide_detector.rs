//! Running IDE Process Detection Engine
//!
//! Scans the OS process table via sysinfo to detect active development
//! environments (VS Code, Cursor, Antigravity, Windsurf, Claude, Zed, JetBrains, etc.).
//! Ensures the singleton daemon stays alive as long as at least one IDE remains running.

use sysinfo::System;

/// Known IDE process binary names (normalized lowercase without .exe suffix).
pub const KNOWN_IDE_BINARIES: &[&str] = &[
    "code",
    "cursor",
    "antigravity",
    "windsurf",
    "claude",
    "trae",
    "zed",
    "positron",
    "opencode",
    "devenv",
    "sublime_text",
    "idea",
    "idea64",
    "pycharm",
    "pycharm64",
    "webstorm",
    "webstorm64",
    "rustrover",
    "rustrover64",
    "goland",
    "goland64",
    "clion",
    "clion64",
    "rider",
    "rider64",
    "studio64",
];

/// Checks if a raw process name matches a recognized IDE.
pub fn is_ide_process_name(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    let base = lower.strip_suffix(".exe").unwrap_or(&lower);

    if KNOWN_IDE_BINARIES.contains(&base) {
        return true;
    }

    if base.starts_with("code-") || base.starts_with("code -") {
        return true;
    }

    false
}

/// Refreshes the system process table and counts how many IDE processes are active.
pub fn count_running_ide_processes(sys: &mut System) -> usize {
    sys.refresh_processes();
    sys.processes()
        .values()
        .filter(|p| is_ide_process_name(p.name()))
        .count()
}

/// Returns true if at least one recognized IDE is currently running.
pub fn has_running_ide_processes(sys: &mut System) -> bool {
    sys.refresh_processes();
    sys.processes()
        .values()
        .any(|p| is_ide_process_name(p.name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ide_process_name() {
        assert!(is_ide_process_name("Code.exe"));
        assert!(is_ide_process_name("code"));
        assert!(is_ide_process_name("code-insiders.exe"));
        assert!(is_ide_process_name("Cursor.exe"));
        assert!(is_ide_process_name("cursor"));
        assert!(is_ide_process_name("Antigravity.exe"));
        assert!(is_ide_process_name("antigravity"));
        assert!(is_ide_process_name("windsurf.exe"));
        assert!(is_ide_process_name("Claude.exe"));
        assert!(is_ide_process_name("idea64.exe"));
        assert!(is_ide_process_name("pycharm64.exe"));
        assert!(is_ide_process_name("rustrover64.exe"));
        assert!(is_ide_process_name("trae.exe"));
        assert!(is_ide_process_name("zed.exe"));

        assert!(!is_ide_process_name("chrome.exe"));
        assert!(!is_ide_process_name("node.exe"));
        assert!(!is_ide_process_name("cargo.exe"));
        assert!(!is_ide_process_name("explorer.exe"));
    }

    #[test]
    fn test_system_new_vs_new_all() {
        let mut sys1 = System::new();
        let count1 = count_running_ide_processes(&mut sys1);

        let mut sys2 = System::new_all();
        let count2 = count_running_ide_processes(&mut sys2);

        eprintln!("System::new() count: {}, System::new_all() count: {}", count1, count2);
        assert_eq!(count1, count2);
    }
}
