use std::fs;
use std::path::PathBuf;

use crate::context::graph_rag::jit_sync::ensure_fresh_targeted_file;
use crate::context::indexer::invalidator::{
    invalidate_and_sync_file, is_file_dirty, sync_dirty_files,
};

fn test_dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "ag_inval_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(p.join("src")).unwrap();
    p
}

#[test]
fn test_is_file_dirty_and_invalidation() {
    let dir = test_dir("dirty_sync");
    let file_rel = "src/service.rs";
    let full_path = dir.join(file_rel);

    fs::write(&full_path, "pub fn execute() -> bool { true }\n").unwrap();

    // Not yet indexed -> dirty
    assert!(is_file_dirty(&dir, file_rel));

    // Sync file
    let rep1 = invalidate_and_sync_file(&dir, file_rel).unwrap();
    assert!(rep1.is_dirty);
    assert!(rep1.reindexed);
    assert!(rep1.symbols_extracted >= 1);

    // Unmodified -> not dirty, no-op reindex
    assert!(!is_file_dirty(&dir, file_rel));
    let rep2 = invalidate_and_sync_file(&dir, file_rel).unwrap();
    assert!(!rep2.is_dirty);
    assert!(!rep2.reindexed);

    // Modify file with different size
    fs::write(
        &full_path,
        "pub fn execute() -> bool { true }\npub fn terminate() {}\n",
    )
    .unwrap();
    assert!(is_file_dirty(&dir, file_rel));

    let rep3 = invalidate_and_sync_file(&dir, file_rel).unwrap();
    assert!(rep3.is_dirty);
    assert!(rep3.reindexed);
    assert!(rep3.symbols_extracted >= 2);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_sync_dirty_files_batch() {
    let dir = test_dir("batch_sync");
    let f1 = "src/a.rs";
    let f2 = "src/b.rs";

    fs::write(dir.join(f1), "pub fn a() {}\n").unwrap();
    fs::write(dir.join(f2), "pub fn b() {}\n").unwrap();

    let files = vec![f1.to_string(), f2.to_string()];
    let reports = sync_dirty_files(&dir, &files).unwrap();
    assert_eq!(reports.len(), 2);
    assert!(reports.iter().all(|r| r.reindexed));

    // Second run without modifications -> empty reports
    let reports2 = sync_dirty_files(&dir, &files).unwrap();
    assert_eq!(reports2.len(), 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_ensure_fresh_targeted_file_flow() {
    let dir = test_dir("jit_targeted");
    let file_rel = "src/worker.rs";
    let full_path = dir.join(file_rel);

    fs::write(&full_path, "pub fn do_work() {}\n").unwrap();

    // First targeted JIT call indexes file
    let res1 = ensure_fresh_targeted_file(&dir, file_rel);
    assert!(res1.is_ok());
    assert!(!is_file_dirty(&dir, file_rel));

    // Modify file
    fs::write(
        &full_path,
        "pub fn do_work() {}\npub fn extra_work() {}\n",
    )
    .unwrap();
    assert!(is_file_dirty(&dir, file_rel));

    // Targeted JIT sync immediately updates file bypassing any debounce
    let res2 = ensure_fresh_targeted_file(&dir, file_rel);
    assert!(res2.is_ok());
    assert!(!is_file_dirty(&dir, file_rel));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_leading_slash_and_deleted_file_invalidation() {
    let dir = test_dir("leading_slash");
    let file_rel = "src/nested/mod.rs";
    fs::create_dir_all(dir.join("src/nested")).unwrap();
    let full_path = dir.join(file_rel);

    fs::write(&full_path, "pub fn helper() -> i32 { 42 }\n").unwrap();

    // Leading slashes should be normalized
    assert!(is_file_dirty(&dir, "/src/nested/mod.rs"));
    assert!(is_file_dirty(&dir, "\\src\\nested\\mod.rs"));

    let rep = invalidate_and_sync_file(&dir, "/src/nested/mod.rs").unwrap();
    assert!(rep.reindexed);
    assert_eq!(rep.rel_path, "src/nested/mod.rs");
    assert!(!is_file_dirty(&dir, "/src/nested/mod.rs"));

    // Deleting file marks it dirty
    fs::remove_file(&full_path).unwrap();
    assert!(is_file_dirty(&dir, file_rel));

    let rep_del = invalidate_and_sync_file(&dir, file_rel).unwrap();
    assert!(rep_del.is_dirty);

    let _ = fs::remove_dir_all(&dir);
}
