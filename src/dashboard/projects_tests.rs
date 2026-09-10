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

#[test]
fn test_find_project_root_with_agent_context() {
    let temp_dir = std::env::temp_dir().join("test_agent_ctx_root");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let proj_root = temp_dir.join("my_app");
    let agent_ctx = proj_root.join(".agent-context");
    let deep_sub = proj_root.join("src").join("dashboard_src").join("js");
    std::fs::create_dir_all(&agent_ctx).unwrap();
    std::fs::create_dir_all(&deep_sub).unwrap();

    let resolved = find_project_root(&deep_sub);
    assert_eq!(resolved, proj_root);

    let norm_sub = normalize_project_path(deep_sub.to_str().unwrap());
    let norm_root = normalize_project_path(proj_root.to_str().unwrap());
    assert_eq!(norm_sub, norm_root);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_list_tracked_projects_consolidates_subfolders() {
    let temp_dir = std::env::temp_dir().join("test_dash_proj_consolidate");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let proj_root = temp_dir.join("web_project");
    let agent_ctx = proj_root.join(".agent-context");
    let sub1 = proj_root.join("src").join("dashboard_src");
    let sub2 = proj_root.join("src").join("dashboard_src").join("js");
    std::fs::create_dir_all(&agent_ctx).unwrap();
    std::fs::create_dir_all(&sub1).unwrap();
    std::fs::create_dir_all(&sub2).unwrap();

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

    conn.execute(
        "INSERT INTO tracked_projects (project_path, project_name, first_seen, last_active, total_calls, total_tokens_saved)
         VALUES (?1, 'web_project', 1000, 2000, 5, 500),
                (?2, 'dashboard_src', 1100, 2500, 3, 300),
                (?3, 'js', 1200, 3000, 2, 200)",
        [
            proj_root.to_str().unwrap(),
            sub1.to_str().unwrap(),
            sub2.to_str().unwrap(),
        ],
    ).unwrap();

    let list = list_tracked_projects(&db_path).unwrap();
    let norm_root = normalize_project_path(proj_root.to_str().unwrap());

    let matches: Vec<_> = list.iter().filter(|p| p.path == norm_root).collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "web_project");
    assert_eq!(matches[0].total_calls, 10);
    assert_eq!(matches[0].tokens_saved, 1000);
    assert_eq!(matches[0].last_active, 3000);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_live_prune_consolidates_existing_db() {
    let db_path = crate::mcp::db::get_db_path();
    if db_path.exists() {
        let _ = prune_missing_projects(&db_path);
        let list = list_tracked_projects(&db_path).unwrap();
        assert!(!list.iter().any(|p| p.path.contains("dashboard_src")));
        assert!(!list.iter().any(|p| p.path.ends_with("Jun")));
    }
}

#[test]
fn test_find_project_root_relative_paths() {
    let cwd = std::env::current_dir().unwrap();
    let norm_cwd = normalize_project_path(&cwd.to_string_lossy());

    let dot = normalize_project_path(".");
    assert_eq!(dot, norm_cwd);

    let src = normalize_project_path("src");
    assert_eq!(src, norm_cwd);
}

#[test]
fn test_is_temp_project_path_trailing_slashes() {
    if let Some(home) = dirs::home_dir() {
        let h1 = format!("{}/", home.to_string_lossy());
        let h2 = format!("{}\\", home.to_string_lossy());
        assert!(is_temp_project_path(&h1));
        assert!(is_temp_project_path(&h2));
    }
}
