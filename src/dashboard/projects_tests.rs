//! Unit tests for dashboard projects tracking, status detection, and pruning.

use super::*;
use rusqlite::Connection;

#[test]
fn test_list_tracked_projects_active_and_missing() {
    let temp_dir = std::env::temp_dir().join("test_dash_proj_status");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let db_path = temp_dir.join("usage.db");
    let conn = Connection::open(&db_path).unwrap();

    conn.execute(
        "CREATE TABLE tracked_projects (
            project_path TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            first_seen INTEGER NOT NULL,
            last_active INTEGER NOT NULL,
            total_calls INTEGER DEFAULT 0,
            total_tokens_saved INTEGER DEFAULT 0
        )",
        [],
    ).unwrap();

    let real_proj = temp_dir.join("active_proj");
    std::fs::create_dir_all(&real_proj).unwrap();
    let real_proj_str = real_proj.to_str().unwrap();

    let fake_proj_str = "C:/fake/path/that/does/not/exist_12345";

    conn.execute(
        "INSERT INTO tracked_projects (project_path, project_name, first_seen, last_active, total_calls, total_tokens_saved)
         VALUES (?1, 'ActiveProj', 1000, 2000, 5, 500),
                (?2, 'MissingProj', 1000, 1500, 2, 200)",
        [real_proj_str, fake_proj_str],
    ).unwrap();

    let list = list_tracked_projects(&db_path).unwrap();
    let norm_real = normalize_project_path(real_proj_str);
    let active_item = list.iter().find(|p| p.path == real_proj_str || p.path == norm_real).expect("Active project found");
    assert_eq!(active_item.status, "active");

    let norm_fake = normalize_project_path(fake_proj_str);
    let missing_item = list.iter().find(|p| p.path == fake_proj_str || p.path == norm_fake).expect("Missing project found");
    assert_eq!(missing_item.status, "missing");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_prune_missing_projects() {
    let temp_dir = std::env::temp_dir().join("test_dash_proj_prune");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let db_path = temp_dir.join("usage.db");
    let conn = Connection::open(&db_path).unwrap();

    conn.execute(
        "CREATE TABLE tracked_projects (
            project_path TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            first_seen INTEGER NOT NULL,
            last_active INTEGER NOT NULL,
            total_calls INTEGER DEFAULT 0,
            total_tokens_saved INTEGER DEFAULT 0
        )",
        [],
    ).unwrap();

    let real_proj = temp_dir.join("active_proj");
    std::fs::create_dir_all(&real_proj).unwrap();
    let real_proj_str = real_proj.to_str().unwrap();

    let fake_proj_str = "C:/fake/path/that/does/not/exist_prune";

    conn.execute(
        "INSERT INTO tracked_projects (project_path, project_name, first_seen, last_active, total_calls, total_tokens_saved)
         VALUES (?1, 'ActiveProj', 1000, 2000, 5, 500),
                (?2, 'MissingProj', 1000, 1500, 2, 200)",
        [real_proj_str, fake_proj_str],
    ).unwrap();

    let pruned = prune_missing_projects(&db_path).unwrap();
    assert_eq!(pruned, 1);

    let list = list_tracked_projects(&db_path).unwrap();
    let norm_real = normalize_project_path(real_proj_str);
    assert!(list.iter().any(|p| p.path == real_proj_str || p.path == norm_real));
    assert!(!list.iter().any(|p| p.path == fake_proj_str));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_normalize_project_path_deduplication() {
    let p1 = normalize_project_path("e:/Github/Agent-Guidance-Rust");
    let p2 = normalize_project_path("E:\\Github\\Agent-Guidance-Rust");
    let p3 = normalize_project_path("e:\\Github\\Agent-Guidance-Rust\\");
    assert_eq!(p1, p2);
    assert_eq!(p2, p3);
    assert!(p1.starts_with("E:"));
}
