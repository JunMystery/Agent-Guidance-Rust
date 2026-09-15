use super::*;
use crate::context::db::CodeGraphDb;
use std::fs;
use std::path::PathBuf;

fn temp_dist_dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "ag_test_dist_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn test_snapshot_export_and_import_flow() {
    let src_proj = temp_dist_dir("src_proj");
    let dst_proj = temp_dist_dir("dst_proj");
    let snapshot_file = temp_dist_dir("snapshots").join("graph.snapshot");

    // Initialize source project database
    let db = CodeGraphDb::open_for_project(&src_proj).unwrap();
    db.upsert_file("src/sample.rs", "h1", 100, 0).unwrap();
    drop(db);

    // 1. Export snapshot
    let checksum = export_graph_snapshot(&src_proj, &snapshot_file).unwrap();
    assert!(!checksum.is_empty());
    assert!(snapshot_file.exists());

    // 2. Import into empty dst project
    import_graph_snapshot(&snapshot_file, &dst_proj, Some(&checksum)).unwrap();

    // 3. Verify destination project has the imported file
    let dst_db = CodeGraphDb::open_for_project(&dst_proj).unwrap();
    let count = dst_db.file_count().unwrap();
    assert_eq!(count, 1);
    drop(dst_db);

    // 4. Test checksum mismatch rejection
    let corrupted_checksum = "0000000000000000";
    let err = import_graph_snapshot(&snapshot_file, &dst_proj, Some(corrupted_checksum));
    assert!(err.is_err());

    let _ = fs::remove_dir_all(&src_proj);
    let _ = fs::remove_dir_all(&dst_proj);
    let _ = fs::remove_file(&snapshot_file);
}
