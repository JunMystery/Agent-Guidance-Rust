//! Graph snapshot export and import with integrity verification.

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use crate::context::db::CodeGraphDb;

/// Computes a deterministic checksum of a file on disk.
pub fn compute_file_checksum(file_path: &Path) -> Result<String> {
    let mut file = fs::File::open(file_path)
        .with_context(|| format!("Cannot open file for checksum: {:?}", file_path))?;
    let mut hasher = DefaultHasher::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.write(&buffer[..n]);
    }

    Ok(format!("{:016x}", hasher.finish()))
}

/// Exports the SQLite code graph database of `project_path` into `out_file`.
/// Returns the snapshot checksum.
pub fn export_graph_snapshot(project_path: &Path, out_file: &Path) -> Result<String> {
    let db_path = project_path.join(".agent-context").join("code_graph.db");
    if !db_path.exists() {
        bail!("Code graph database does not exist at {:?}", db_path);
    }

    // Flush WAL to ensure complete snapshot
    if let Ok(db) = CodeGraphDb::open_for_project(project_path) {
        let _ = db.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }

    if let Some(parent) = out_file.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(&db_path, out_file)
        .with_context(|| format!("Failed to copy snapshot from {:?} to {:?}", db_path, out_file))?;

    compute_file_checksum(out_file)
}

/// Imports and verifies a snapshot into `project_path`, replacing existing database.
pub fn import_graph_snapshot(
    snapshot_file: &Path,
    project_path: &Path,
    expected_checksum: Option<&str>,
) -> Result<()> {
    if !snapshot_file.exists() {
        bail!("Snapshot file not found: {:?}", snapshot_file);
    }

    // 1. Verify Checksum
    let actual_checksum = compute_file_checksum(snapshot_file)?;
    if let Some(expected) = expected_checksum {
        if actual_checksum != expected {
            bail!(
                "Snapshot checksum mismatch: expected {}, got {}",
                expected,
                actual_checksum
            );
        }
    }

    // 2. Verify SQLite integrity before installing
    let verify_conn = Connection::open_with_flags(
        snapshot_file,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let integrity: String = verify_conn.query_row("PRAGMA quick_check;", [], |r| r.get(0))?;
    if integrity != "ok" {
        bail!("Snapshot SQLite integrity check failed: {}", integrity);
    }
    drop(verify_conn);

    // 3. Atomically install into target directory
    let agent_ctx = project_path.join(".agent-context");
    fs::create_dir_all(&agent_ctx)?;
    let target_db = agent_ctx.join("code_graph.db");

    let temp_target = agent_ctx.join("code_graph.db.incoming");
    fs::copy(snapshot_file, &temp_target)?;
    fs::rename(&temp_target, &target_db)?;

    // Remove old WAL and SHM if present
    let _ = fs::remove_file(agent_ctx.join("code_graph.db-wal"));
    let _ = fs::remove_file(agent_ctx.join("code_graph.db-shm"));

    Ok(())
}
