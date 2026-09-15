use std::path::Path;
use crate::dashboard::projects_path::normalize_project_path;
use crate::mcp::state::ServerState;
use super::tools::validate_path;

#[test]
fn test_validate_path_leading_slashes() {
    let base = Path::new(".");
    let res1 = validate_path(base, "/Cargo.toml");
    assert!(res1.is_ok(), "Leading slash should resolve inside workspace");
    let res2 = validate_path(base, "\\Cargo.toml");
    assert!(res2.is_ok(), "Leading backslash should resolve inside workspace");
    let res3 = validate_path(base, "///Cargo.toml");
    assert!(res3.is_ok(), "Multiple leading slashes should resolve inside workspace");
}

#[test]
fn test_validate_path_cross_platform_slashes() {
    let base = Path::new(".");
    let res1 = validate_path(base, "src\\main.rs");
    assert!(res1.is_ok(), "Windows slash should resolve on all platforms");

    let res2 = validate_path(base, "/src/main.rs");
    assert!(res2.is_ok(), "Leading slash with forward slashes should resolve");

    let res3 = validate_path(base, "\\src\\main.rs");
    assert!(res3.is_ok(), "Leading backslash with Windows slashes should resolve");
}

#[test]
fn test_validate_path_absolute_within_workspace() {
    let base = Path::new(".");
    let abs_cargo = base.canonicalize().unwrap().join("Cargo.toml");
    let res = validate_path(base, abs_cargo.to_str().unwrap());
    assert!(res.is_ok(), "Absolute path inside workspace should be accepted");

    // Non-canonical absolute path (lacking \\?\ on Windows)
    let cwd_cargo = std::env::current_dir().unwrap().join("Cargo.toml");
    let res2 = validate_path(base, cwd_cargo.to_str().unwrap());
    assert!(res2.is_ok(), "Standard absolute path should match when base is relative dot");
}

#[test]
fn test_validate_path_blocks_outside_and_traversal() {
    let base = Path::new(".");
    assert!(validate_path(base, "../../etc/passwd").is_err());
    assert!(validate_path(base, "..\\..\\etc\\passwd").is_err());

    #[cfg(windows)]
    {
        assert!(validate_path(base, "C:\\Windows\\System32\\cmd.exe").is_err());
    }
    #[cfg(not(windows))]
    {
        assert!(validate_path(base, "/etc/passwd").is_err());
    }
}

#[test]
fn test_normalize_project_path_unix_and_windows() {
    // Windows drive path
    let win = normalize_project_path("c:/projects/app///");
    assert!(win.starts_with("C:"), "Windows drive letter should be uppercased");
    assert!(!win.ends_with('/'), "Trailing slashes should be stripped");
    assert!(!win.ends_with('\\'), "Trailing backslashes should be stripped");

    // Unix style path
    #[cfg(not(windows))]
    {
        let unix = normalize_project_path("/home/user/projects/repo///");
        assert!(!unix.contains('\\'), "Unix path must never contain backslashes");
        assert!(unix.starts_with('/'), "Unix path must preserve root slash");
        assert!(!unix.ends_with('/'), "Trailing slash should be stripped");
    }
}

#[test]
fn test_record_modified_file_leading_slashes() {
    let mut state = ServerState::new();
    state.record_modified_file("/src/service.rs");
    state.record_modified_file("\\src\\repo.rs");
    assert_eq!(state.modified_files, vec!["src/service.rs", "src/repo.rs"]);
    assert_eq!(state.dirty_files, vec!["src/service.rs", "src/repo.rs"]);
}
