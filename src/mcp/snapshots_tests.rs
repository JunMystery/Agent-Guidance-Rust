use std::fs;
use super::snapshots::*;

#[test]
fn test_create_and_restore_snapshots() {
    let temp_dir = std::env::temp_dir().join(format!("snap_test_basic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir);

    let file_rel = "src/example.rs";
    let file_full = temp_dir.join(file_rel);
    let _ = fs::create_dir_all(file_full.parent().unwrap());
    fs::write(&file_full, "original content").unwrap();

    let session_id = "session_test_1";

    // 1. Create snapshot
    let created = create_file_snapshot(&temp_dir, file_rel, session_id).unwrap();
    assert!(created, "Snapshot should be created for first edit");

    // 2. Modify original file
    fs::write(&file_full, "modified corrupted content").unwrap();
    assert_eq!(fs::read_to_string(&file_full).unwrap(), "modified corrupted content");

    // 3. Subsequent snapshot in same session should NOT overwrite pristine copy
    let second_call = create_file_snapshot(&temp_dir, file_rel, session_id).unwrap();
    assert!(!second_call, "Second snapshot in same session should return false (already exists)");

    // 4. Restore snapshot
    let restored = restore_session_snapshots(&temp_dir, session_id).unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(fs::read_to_string(&file_full).unwrap(), "original content");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cleanup_stale_snapshots_lru_cap() {
    let temp_dir = std::env::temp_dir().join(format!("snap_test_lru_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir);

    let snapshots_base = temp_dir.join(".agent-context").join("snapshots");

    // Create 25 session snapshot directories
    for i in 1..=25 {
        let sess_dir = snapshots_base.join(format!("session_{:02}", i));
        fs::create_dir_all(&sess_dir).unwrap();
        fs::write(sess_dir.join("sample.rs"), format!("// session {}", i)).unwrap();
    }

    // Cleanup enforcing max 20 sessions (with large age days so LRU triggers)
    let pruned = cleanup_stale_snapshots(&temp_dir, 365, 20);
    assert_eq!(pruned, 5, "Should prune exactly 5 excess sessions to maintain cap 20");

    let remaining_count = fs::read_dir(&snapshots_base)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .count();
    assert_eq!(remaining_count, 20, "Should have exactly 20 remaining snapshot directories");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_clear_all_snapshots() {
    let temp_dir = std::env::temp_dir().join(format!("snap_test_clear_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir);

    let snapshots_base = temp_dir.join(".agent-context").join("snapshots");
    fs::create_dir_all(snapshots_base.join("sess_abc")).unwrap();
    fs::write(snapshots_base.join("sess_abc").join("file.rs"), "data").unwrap();

    assert!(snapshots_base.exists());

    clear_all_snapshots(&temp_dir).unwrap();
    assert!(!snapshots_base.exists(), "snapshots directory should be completely removed");

    let _ = fs::remove_dir_all(&temp_dir);
}
